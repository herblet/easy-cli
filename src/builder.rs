use std::ops::{Range, RangeFrom, RangeTo};
use std::path::PathBuf;

use nom::{
    Compare, InputIter, InputLength, InputTakeAtPosition, IResult, Parser, sequence::preceded,
    Slice,
};
use nom::branch::alt;
use nom::bytes::complete::tag_no_case;
use nom::bytes::streaming::is_not;
use nom::character::complete::anychar;
use nom::character::streaming::{multispace0, not_line_ending, space0};
use nom::combinator::{flat_map, iterator, map, opt, rest, value};
use nom::Err::{Error, Failure, Incomplete};
use nom::error::ParseError;
use nom::sequence::{delimited, pair, terminated, tuple};

use crate::model::{ArgType, Command, CommandArg, CommandOption, EmbeddedCommand, ScriptCommand};
use crate::model::ArgType::Unknown;
use crate::utils::strip_file_suffix;

const TRUE: &'static str = "true";
const FALSE: &'static str = "false";

const IGNORE_TAG: &'static str = "ignore";
const SUB_TAG: &'static str = "sub";
const NAME_TAG: &'static str = "name";
const ABOUT_TAG: &'static str = "about";
const ARG_TAG: &'static str = "arg";
const VAR_ARG_TAG: &'static str = "vararg";
const OPT_TAG: &'static str = "opt";

#[derive(Debug, Clone, PartialEq)]
struct NameTag {
    name: String,
}

impl NameTag {
    fn new(name: String) -> Self {
        NameTag { name }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct SubTag {
    name: String,
    path: Option<String>,
}

impl SubTag {
    fn new(name: String, path: Option<String>) -> Self {
        SubTag { name, path }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct AboutTag {
    text: String,
}

impl AboutTag {
    fn new(text: String) -> Self {
        AboutTag { text }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum DocTag {
    Ignore,
    Name(NameTag),
    Sub(SubTag),
    About(AboutTag),
    Arg(CommandArg),
    Opt(CommandOption),
}

trait FinishIncomplete<T, O, E> {
    fn finish_with_val(self, value: O) -> Result<O, E>;
}

impl<T, O1, O2, E> FinishIncomplete<T, O1, E> for IResult<T, O2, E> {
    fn finish_with_val(self, value: O1) -> Result<O1, E> {
        match self {
            Ok((_, _)) => Ok(value),
            Err(Incomplete(_)) => Ok(value),
            Err(Error(e)) => Err(e),
            Err(Failure(e)) => Err(e),
        }
    }
}

pub trait InputType:
    InputTakeAtPosition<Item = char>
    + Slice<Range<usize>>
    + Slice<RangeFrom<usize>>
    + Slice<RangeTo<usize>>
    + InputIter<Item = char>
    + InputLength
    + Compare<&'static str>
    + ToString
{
}

impl InputType for &str {}

fn padded<'a, T: InputType + 'a, O, E: ParseError<T>, F>(
    parser: F,
) -> impl FnMut(T) -> IResult<T, O, E>
where
    F: Parser<T, O, E>,
{
    preceded(space0, parser)
}

fn identifier<'a, T: InputType + 'a, E: ParseError<T> + 'a>(input: T) -> IResult<T, T, E> {
    is_not(" \t\r\n-")(input)
}

fn ignore_tag<'a, T: InputType + 'a, E: ParseError<T> + 'a>(
    input: T,
) -> IResult<T, Option<DocTag>, E> {
    value(Some(DocTag::Ignore), not_line_ending)(input)
}

fn name_tag<'a, T: InputType + 'a, E: ParseError<T> + 'a>(
    input: T,
) -> IResult<T, Option<DocTag>, E> {
    terminated(preceded(multispace0, identifier), not_line_ending)(input)
        .map(|(i, o)| (i, Some(DocTag::Name(NameTag::new(o.to_string())))))
}

fn sub_tag<'a, T: InputType + 'a, E: ParseError<T> + 'a>(
    input: T,
) -> IResult<T, Option<DocTag>, E> {
    terminated(preceded(multispace0, identifier), not_line_ending)(input)
        .map(|(i, o)| (i, Some(DocTag::Sub(SubTag::new(o.to_string(), None)))))
}

fn about_tag<'a, T: InputType + 'a, E: ParseError<T> + 'a>(
    input: T,
) -> IResult<T, Option<DocTag>, E> {
    padded(not_line_ending)(input)
        .map(|(i, o)| (i, Some(DocTag::About(AboutTag::new(o.to_string())))))
}

fn arg_tag<'a, T: InputType + 'a, E: ParseError<T> + 'a>(
    input: T,
) -> IResult<T, Option<DocTag>, E> {
    arg_var_arg(false, input)
}

fn arg_var_arg<'a, T: InputType + 'a, E: ParseError<T> + 'a>(
    var_arg: bool,
    input: T,
) -> IResult<T, Option<DocTag>, E> {
    preceded(multispace0, pair(identifier, padded(not_line_ending)))(input).map(|(i, o)| {
        let name = o.0.to_string();
        let details = o.1.to_string();
        let inner_res =
            arg_details::<nom::error::Error<&str>>(name.as_str(), var_arg, details.as_str());

        (i, inner_res.map(|(_, o)| o).unwrap_or(None::<DocTag>))
    })
}

fn var_arg_tag<'a, T: InputType + 'a, E: ParseError<T> + 'a>(
    input: T,
) -> IResult<T, Option<DocTag>, E> {
    arg_var_arg(true, input)
}

fn arg_type<'a, E: ParseError<&'a str> + 'a>(input: &'a str) -> IResult<&'a str, ArgType, E> {
    preceded(
        nom::character::complete::space0,
        delimited(
            nom::character::complete::char('<'),
            map(is_not(">"), ArgType::from),
            nom::character::complete::char('>'),
        ),
    )(input)
}

fn arg_details<'a, E: ParseError<&'a str> + 'a>(
    name: &'a str,
    var_arg: bool,
    input: &'a str,
) -> IResult<&'a str, Option<DocTag>, E> {
    map(
        tuple((
            opt(padded_bool),
            opt(arg_type),
            preceded(nom::character::complete::space0, rest),
        )),
        |(optional, arg_type, rest)| {
            Some(DocTag::Arg(CommandArg::new(
                name.to_string(),
                optional.unwrap_or(false),
                var_arg,
                arg_type.unwrap_or(Unknown),
                none_if_empty(rest),
            )))
        },
    )(input)
}

fn none_if_empty(rest: &str) -> Option<String> {
    if rest.input_len() > 0 {
        Some(rest.to_string())
    } else {
        None
    }
}

fn opt_tag<'a, T: InputType + 'a, E: ParseError<T> + 'a>(
    input: T,
) -> IResult<T, Option<DocTag>, E> {
    preceded(multispace0, pair(identifier, padded(not_line_ending)))(input).map(|(i, o)| {
        let name = o.0.to_string();
        let details = o.1.to_string();

        let inner_res = opt_details::<nom::error::Error<&str>>(name.as_str(), details.as_str());

        (i, inner_res.map(|(_, o)| o).unwrap_or(None::<DocTag>))
    })
}

fn opt_details<'a, E: ParseError<&'a str> + 'a>(
    name: &'a str,
    input: &'a str,
) -> IResult<&'a str, Option<DocTag>, E> {
    map(
        preceded(
            nom::character::complete::space0,
            tuple((
                opt(delimited(
                    nom::character::complete::char('\''),
                    anychar::<&'a str, _>,
                    nom::character::complete::char('\''),
                )),
                padded_bool_default_false,
                preceded(nom::character::complete::space0, rest),
            )),
        ),
        |(short, has_param, rest)| {
            Some(DocTag::Opt(CommandOption::new(
                name.to_string(),
                short,
                has_param,
                none_if_empty(rest),
            )))
        },
    )(input)
}

fn padded_bool_default_false<'a, E: ParseError<&'a str> + 'a>(
    input: &'a str,
) -> IResult<&'a str, bool, E> {
    map(opt(padded_bool), |x| x.unwrap_or(false))(input)
}

fn padded_bool<'a, E: ParseError<&'a str> + 'a>(input: &'a str) -> IResult<&'a str, bool, E> {
    preceded(
        nom::character::complete::space0,
        alt((
            value(true, tag_no_case(TRUE)),
            value(false, tag_no_case(FALSE)),
        )),
    )(input)
}

fn unknown_tag<'a, T: InputType + 'a, E: ParseError<T> + 'a>(
    input: T,
) -> IResult<T, Option<DocTag>, E> {
    value(None, not_line_ending)(input)
}

fn parser_for_tag<'a, T: InputType + 'a, E: ParseError<T> + 'a>(
    tag: T,
) -> Box<dyn Parser<T, Option<DocTag>, E> + 'a> {
    match tag.to_string().as_str() {
        IGNORE_TAG => Box::new(ignore_tag),
        NAME_TAG => Box::new(name_tag),
        SUB_TAG => Box::new(sub_tag),
        ABOUT_TAG => Box::new(about_tag),
        ARG_TAG => Box::new(arg_tag),
        VAR_ARG_TAG => Box::new(var_arg_tag),
        OPT_TAG => Box::new(opt_tag),
        _ => Box::new(unknown_tag),
    }
}

fn doc_tag<'a, T: InputType + 'a, E: ParseError<T> + 'a>(
    input: T,
) -> IResult<T, Option<DocTag>, E> {
    flat_map(identifier, parser_for_tag)(input)
}

fn doc_tag_or_not<'a, T: InputType + 'a, E: ParseError<T> + 'a>(
    input: T,
) -> IResult<T, Option<DocTag>, E> {
    flat_map(preceded(multispace0, anychar), |c| {
        if c == '@' {
            doc_tag
        } else {
            unknown_tag
        }
    })(input)
}

fn comment_or_not<'a, T: InputType + 'a, E: ParseError<T> + 'a>(
    input: T,
) -> IResult<T, Option<DocTag>, E> {
    flat_map(preceded(multispace0, anychar), |c| {
        if c == '#' {
            doc_tag_or_not
        } else {
            unknown_tag
        }
    })(input)
}

fn collect<'a, T: InputType + Clone + 'a, E: ParseError<T> + 'a>(
    input: T,
) -> Result<Vec<Vec<DocTag>>, E> {
    // create an iterator over all tags in the input
    let mut iter = iterator(input, comment_or_not);

    // fold the tags into groups of tags, starting a new group when a sub tag is found
    let groups = iter
        .filter_map(|a| a)
        .fold(vec![vec![]], |mut groups, tag| {
            match tag {
                DocTag::Sub(_) => groups.push(vec![tag]),
                _ => groups.last_mut().unwrap().push(tag),
            }
            groups
        });

    iter.finish().finish_with_val(groups)
}

fn default_name(path: &PathBuf) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .map(|file_name| strip_file_suffix(&file_name))
        .unwrap()
}

pub fn build_script_command(path: PathBuf) -> Result<Option<ScriptCommand>, String> {
    let mut file_content = std::fs::read_to_string(&path).unwrap();

    // Until streaming is implemented properly and we can handle incomplete, make sure the file
    // ends with a newline, otherwise we may miss the last tag
    if !file_content.ends_with("\n") {
        file_content.push('\n')
    }

    let res = collect::<&str, nom::error::Error<&str>>(&file_content)
        .map_err(|e| e.to_string())
        .map(|groups| {
            if groups.len() == 0 || groups.len() == 1 && groups[0].len() == 0 {
                // There are no doc-tags. Assume the file is a script
                // and let it accept any args
                Ok(Some(ScriptCommand::new(
                    default_name(&path),
                    None,
                    path,
                    vec![],
                    vec![CommandArg::new(
                        "args",
                        true,
                        true,
                        ArgType::Unknown,
                        Some("Any arguments are passed to the script"),
                    )],
                    vec![],
                )))
            } else if groups[0].len() > 0 && groups[0][0] == DocTag::Ignore {
                Ok(None)
            } else {
                let mut iter = groups.into_iter();

                let main_tags = iter.next().unwrap();

                let mut opts = Vec::new();
                let mut args = Vec::new();

                let mut description = None;
                let mut name = None;

                main_tags.into_iter().for_each(|tag| match tag {
                    DocTag::Arg(arg) => args.push(arg),
                    DocTag::Opt(opt) => opts.push(opt),
                    DocTag::About(about) => description = Some(about.text),
                    DocTag::Name(name_tag) => name = Some(name_tag.name),
                    _ => {}
                });

                let sub_commands = iter
                    .map(|group| {
                        let mut opts = Vec::new();
                        let mut args = Vec::new();

                        let mut group_iter = group.into_iter();

                        let sub_tag = match group_iter.next() {
                            Some(DocTag::Sub(sub)) => sub,
                            _ => return Err("No sub tag found".to_string()),
                        };

                        let mut description = None;

                        group_iter.for_each(|tag| match tag {
                            DocTag::Arg(arg) => args.push(arg),
                            DocTag::Opt(opt) => opts.push(opt),
                            DocTag::About(about) => description = Some(about.text),
                            _ => {}
                        });
                        Ok(EmbeddedCommand::new(sub_tag.name, description, opts, args))
                    })
                    .fold(
                        Ok::<Vec<Box<dyn Command>>, String>(vec![]),
                        |acc, res| match acc {
                            Ok(mut vec) => match res {
                                Ok(val) => {
                                    vec.push(Box::new(val));
                                    Ok(vec)
                                }
                                Err(e) => Err(e),
                            },
                            Err(e) => Err(e),
                        },
                    );

                sub_commands.map(|sub_commands| {
                    Some(ScriptCommand::new(
                        name.unwrap_or(default_name(&path)),
                        description,
                        path,
                        opts,
                        args,
                        sub_commands,
                    ))
                })
            }
        });

    match res {
        Ok(Ok(command)) => Ok(command),
        Ok(Err(e)) => Err(e),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod test {
    use std::fs::File;
    use std::io::Write;

    use indoc::indoc;

    use crate::builder::{
        AboutTag, arg_tag, build_script_command, collect, comment_or_not, doc_tag, doc_tag_or_not,
        DocTag, opt_tag, sub_tag, SubTag, var_arg_tag,
    };
    use crate::model::{ArgType, Command, CommandArg, CommandOption};
    use crate::model::test::NO_DESCRIPTION;

    #[test]
    fn sub_tag_finds_name() {
        let input = indoc! {"
            fooBar ignored
            "};

        let res = sub_tag::<&str, nom::error::Error<&str>>(input);

        let (_, sub) = res.unwrap();

        assert_eq!(
            sub.unwrap(),
            DocTag::Sub(SubTag::new("fooBar".to_string(), None))
        );
    }

    #[test]
    fn tag_finds_sub_tag() {
        let input = indoc! {"
            sub fooBar
            "};

        let res = doc_tag::<&str, nom::error::Error<&str>>(input);

        let (_, sub) = res.unwrap();

        assert_eq!(
            sub.unwrap(),
            DocTag::Sub(SubTag::new("fooBar".to_string(), None))
        );
    }

    #[test]
    fn arg_finds_name() {
        let input = indoc! {"
            fooBar
            "};

        let res = arg_tag::<&str, nom::error::Error<&str>>(input);

        let (_, sub) = res.unwrap();

        assert_eq!(
            sub.unwrap(),
            DocTag::Arg(CommandArg::new(
                "fooBar".to_string(),
                false,
                false,
                ArgType::Unknown,
                NO_DESCRIPTION
            ))
        );
    }

    #[test]
    fn arg_finds_description() {
        let input = indoc! {"
            fooBar A description of foobar
            "};

        let res = arg_tag::<&str, nom::error::Error<&str>>(input);

        let (_, sub) = res.unwrap();

        assert_eq!(
            sub.unwrap(),
            DocTag::Arg(CommandArg::new(
                "fooBar".to_string(),
                false,
                false,
                ArgType::Unknown,
                Some("A description of foobar".to_string()),
            ))
        );
    }

    #[test]
    fn arg_finds_optional() {
        let input = indoc! {"
            fooBar true
            "};

        let res = arg_tag::<&str, nom::error::Error<&str>>(input);

        let (_, sub) = res.unwrap();

        assert_eq!(
            sub.unwrap(),
            DocTag::Arg(CommandArg::new(
                "fooBar".to_string(),
                true,
                false,
                ArgType::Unknown,
                NO_DESCRIPTION
            ))
        );
    }

    #[test]
    fn arg_finds_optional_and_desc() {
        let input = indoc! {"
            fooBar TrUe A description of foobar
            "};

        let res = arg_tag::<&str, nom::error::Error<&str>>(input);

        let (_, sub) = res.unwrap();

        assert_eq!(
            sub.unwrap(),
            DocTag::Arg(CommandArg::new(
                "fooBar".to_string(),
                true,
                false,
                ArgType::Unknown,
                Some("A description of foobar".to_string()),
            ))
        );
    }

    #[test]
    fn arg_finds_type() {
        let input = indoc! {"
            fooBar <file>
            "};

        let res = arg_tag::<&str, nom::error::Error<&str>>(input);

        let (_, sub) = res.unwrap();

        assert_eq!(
            sub.unwrap(),
            DocTag::Arg(CommandArg::new(
                "fooBar".to_string(),
                false,
                false,
                ArgType::File,
                NO_DESCRIPTION
            ))
        );
    }

    #[test]
    fn var_arg_finds_optional_and_desc() {
        let input = indoc! {"
            fooBar TrUe A description of foobar
            "};

        let res = var_arg_tag::<&str, nom::error::Error<&str>>(input);

        let (_, sub) = res.unwrap();

        assert_eq!(
            sub.unwrap(),
            DocTag::Arg(CommandArg::new(
                "fooBar".to_string(),
                true,
                true,
                ArgType::Unknown,
                Some("A description of foobar".to_string()),
            ))
        );
    }

    #[test]
    fn arg_finds_optional_and_type_and_desc() {
        let input = indoc! {"
            fooBar true <file> An optional file
            "};

        let res = arg_tag::<&str, nom::error::Error<&str>>(input);

        let (_, sub) = res.unwrap();

        assert_eq!(
            sub.unwrap(),
            DocTag::Arg(CommandArg::new(
                "fooBar".to_string(),
                true,
                false,
                ArgType::File,
                Some("An optional file".to_string())
            ))
        );
    }

    #[test]
    fn tag_finds_about() {
        let input = indoc! {"
            about This is a description
            "};

        let res = doc_tag::<&str, nom::error::Error<&str>>(input);

        let (_, sub) = res.unwrap();

        assert_eq!(
            sub.unwrap(),
            DocTag::About(AboutTag::new("This is a description".to_string()))
        );
    }

    #[test]
    fn tag_or_not_finds_tag() {
        let input = indoc! {"
            @about This is a description
            "};

        let res = doc_tag_or_not::<&str, nom::error::Error<&str>>(input);

        let (_, sub) = res.unwrap();

        assert_eq!(
            sub.unwrap(),
            DocTag::About(AboutTag::new("This is a description".to_string()))
        );
    }

    #[test]
    fn tag_or_not_ignores_non_tag() {
        let input = indoc! {"
            This is a description
            "};

        let res = doc_tag_or_not::<&str, nom::error::Error<&str>>(input);

        let (_, sub) = res.unwrap();

        assert!(sub.is_none());
    }

    #[test]
    fn comment_or_not_finds_tag() {
        let input = indoc! {"
            # @about This is a description
            "};

        let res = comment_or_not::<&str, nom::error::Error<&str>>(input);

        let (_, sub) = res.unwrap();

        assert_eq!(
            sub.unwrap(),
            DocTag::About(AboutTag::new("This is a description".to_string()))
        );
    }

    #[test]
    fn comment_or_not_ignores_non_tag() {
        let input = indoc! {"
            # This is a description
            "};

        let res = comment_or_not::<&str, nom::error::Error<&str>>(input);

        let (_, sub) = res.unwrap();

        assert!(sub.is_none());
    }

    #[test]
    fn comment_or_not_ignores_non_comment() {
        let input = indoc! {"
            This is a description
            "};

        let res = comment_or_not::<&str, nom::error::Error<&str>>(input);

        let (_, sub) = res.unwrap();

        assert!(sub.is_none());
    }

    #[test]
    fn opt_tag_finds_name() {
        let input = indoc! {"
            fooBar
            "};

        let res = opt_tag::<&str, nom::error::Error<&str>>(input);

        let (_, sub) = res.unwrap();

        assert_eq!(
            sub.unwrap(),
            DocTag::Opt(CommandOption::new(
                "fooBar".to_string(),
                None,
                false,
                NO_DESCRIPTION
            ))
        );
    }

    #[test]
    fn opt_tag_finds_short() {
        let input = indoc! {"
            fooBar 'f'
            "};

        let res = opt_tag::<&str, nom::error::Error<&str>>(input);

        let (_, sub) = res.unwrap();

        assert_eq!(
            sub.unwrap(),
            DocTag::Opt(CommandOption::new(
                "fooBar".to_string(),
                Some('f'),
                false,
                NO_DESCRIPTION,
            ))
        );
    }

    #[test]
    fn opt_tag_finds_has_args() {
        let input = indoc! {"
            fooBar true
            "};

        let res = opt_tag::<&str, nom::error::Error<&str>>(input);

        let (_, sub) = res.unwrap();

        assert_eq!(
            sub.unwrap(),
            DocTag::Opt(CommandOption::new(
                "fooBar".to_string(),
                None,
                true,
                NO_DESCRIPTION
            ))
        );
    }

    #[test]
    fn opt_tag_finds_short_and_has_args() {
        let input = indoc! {"
            fooBar 'd' true
            "};

        let res = opt_tag::<&str, nom::error::Error<&str>>(input);

        let (_, sub) = res.unwrap();

        assert_eq!(
            sub.unwrap(),
            DocTag::Opt(CommandOption::new(
                "fooBar".to_string(),
                Some('d'),
                true,
                NO_DESCRIPTION,
            ))
        );
    }

    #[test]
    fn opt_tag_finds_desc() {
        let input = indoc! {"
            fooBar This param
            "};

        let res = opt_tag::<&str, nom::error::Error<&str>>(input);

        let (_, sub) = res.unwrap();

        assert_eq!(
            sub.unwrap(),
            DocTag::Opt(CommandOption::new(
                "fooBar".to_string(),
                None,
                false,
                Some("This param".to_string()),
            ))
        );
    }

    #[test]
    fn opt_tag_finds_all() {
        let input = indoc! {"
            fooBar 'e' true This param
            "};

        let res = opt_tag::<&str, nom::error::Error<&str>>(input);

        let (_, sub) = res.unwrap();

        assert_eq!(
            sub.unwrap(),
            DocTag::Opt(CommandOption::new(
                "fooBar".to_string(),
                Some('e'),
                true,
                Some("This param".to_string()),
            ))
        );
    }

    #[test]
    fn opt_tag_acccepts_single_letter_at_start_of_desc() {
        let input = indoc! {"
            fooBar A great option
            "};

        let res = opt_tag::<&str, nom::error::Error<&str>>(input);

        let (_, sub) = res.unwrap();

        assert_eq!(
            sub.unwrap(),
            DocTag::Opt(CommandOption::new(
                "fooBar".to_string(),
                None,
                false,
                Some("A great option".to_string()),
            ))
        );
    }

    #[test]
    fn collect_groups_each_subtag() {
        let input = indoc! {"
            # @about This is a description
            # @wrong sdlklak askdölak
            sdjfös lkäösdlföls
            # @sub fooBar
            # @about foobar description
            aöslkfölas
            # fpskdf
            asdssd
            # @sub barFoo
            # @about barFoo description
            aöslkfölas
            # fpskdf
            asdssd
            "};

        let res = collect::<&str, nom::error::Error<&str>>(input);

        let sub = res.unwrap();

        assert_eq!(sub.len(), 3);
    }

    #[test]
    fn build_script_command_includes_sub_commands() {
        let test_dir = tempfile::tempdir().unwrap();

        let script1_path = test_dir.path().join("foo.sh");

        File::create(&script1_path)
            .unwrap()
            .write(
                indoc! {"\
            # @name CommandName blah blah
            # @about foo bar
            # @sub sub1
            # @opt longname 'l' true The description of longname
            function sub1(){}
            # @sub sub2
            # @arg arg1 true The description of arg1
            function sub2(){}
            "}
                .as_bytes(),
            )
            .expect(format!("Unable to create file {}", script1_path.to_str().unwrap()).as_str());

        let command = build_script_command(script1_path).unwrap().unwrap();

        assert_eq!(command.name, "CommandName");
        assert_eq!(command.description, Some("foo bar".to_string()));

        let sub_commands = command.sub_commands();
        assert_eq!(sub_commands.len(), 2);
        assert_eq!(sub_commands[0].name(), "sub1");
        assert_eq!(sub_commands[0].options().len(), 1);

        let option = &sub_commands[0].options()[0];

        assert_eq!(option.name, "longname");
        assert_eq!(option.short, Some('l'));
        assert_eq!(option.has_param, true);
        assert_eq!(
            option.description,
            Some("The description of longname".to_string())
        );

        assert_eq!(sub_commands[1].name(), "sub2");
        assert_eq!(sub_commands[1].args().len(), 1);

        let arg = &sub_commands[1].args()[0];

        assert_eq!(arg.name, "arg1");
        assert_eq!(arg.optional, true);
        assert_eq!(arg.var_arg, false);
        assert_eq!(arg.description, Some("The description of arg1".to_string()));
    }

    #[test]
    fn build_script_command_finds_tag_on_last_line() {
        let test_dir = tempfile::tempdir().unwrap();

        let script1_path = test_dir.path().join("foo.sh");

        File::create(&script1_path)
            .unwrap()
            .write(
                indoc! {"\
                # @about The description of this file"}
                .as_bytes(),
            )
            .expect(format!("Unable to create file {}", script1_path.to_str().unwrap()).as_str());

        let command = build_script_command(script1_path).unwrap().unwrap();

        assert_eq!(command.name, "foo");
        assert_eq!(
            command.description.unwrap().as_str(),
            "The description of this file"
        );
    }
}
