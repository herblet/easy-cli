
use std::{io::{Read, BufReader, BufRead}, str::FromStr};

#[derive(Debug, PartialEq)]
pub enum DocEntry {
    Name(String),
    Description(String),
    Argument(String),
    Option(String, bool, String),
    AnyArg
}

pub fn doc_entries<T: Read>(reader: BufReader<T>) -> Vec<DocEntry> {
    reader.lines().filter(Result::is_ok)
        .map(Result::unwrap)
        .filter(|line| line.starts_with("#@"))
        .map(|line| to_doc_entry(line))
        .collect()
}

fn to_doc_entry(line: String) -> DocEntry {
    let mut parts = line.splitn(2, " ");

    let entry_type = parts.next().unwrap();
    let entry_value = parts.next();

    match entry_type {
        "#@name" => DocEntry::Name(entry_value.unwrap().to_owned()),
        "#@description" => DocEntry::Description(entry_value.unwrap().to_owned()),
        "#@argument" => DocEntry::Argument(entry_value.unwrap().to_owned()),
        "#@anyarg" => DocEntry::AnyArg,
        "#@option" => {
            let mut parts = entry_value.unwrap().splitn(3, " ");

            let option_name = parts.next().unwrap();
            let option_has_arg = FromStr::from_str(parts.next().unwrap()).unwrap_or(false);
            let option_description = parts.next().unwrap();

            DocEntry::Option(option_name.to_owned(), option_has_arg, option_description.to_owned())
        }
        _ => panic!("Unknown doc entry type: {}", entry_type),
    }
}

#[cfg(test)]
mod test {
    use indoc::indoc;
    use stringreader::StringReader;

    use super::*;

    #[test]
    fn parses_description() {
        let input = BufReader::new(StringReader::new(
            indoc!{"
            name: foo
            # hello 
            #@description bar"}));
        let result = doc_entries(input);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], DocEntry::Description("bar".to_owned()));

    }

     #[test]
    fn parses_options() {
        let input = BufReader::new(StringReader::new(
            indoc!{"
            name: foo
            # hello 
            #@option stop false Stops the world.
            "}));
        let result = doc_entries(input);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], DocEntry::Option("stop".to_owned(), false, "Stops the world.".to_owned()));

    }
}
