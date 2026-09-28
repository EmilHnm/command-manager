use crate::error::{Error, Result};

/// Split a direct-argv command line without applying the quoting rules of a
/// Unix shell to Windows paths. On Windows this follows the escaping rules
/// used by CommandLineToArgvW: backslashes are preserved unless they are
/// immediately followed by a quote.
pub fn split_argv(input: &str) -> Result<Vec<String>> {
    #[cfg(windows)]
    {
        split_windows_argv(input)
    }

    #[cfg(not(windows))]
    {
        shell_words::split(input).map_err(|error| Error::Argv(error.to_string()))
    }
}

#[cfg(windows)]
fn split_windows_argv(input: &str) -> Result<Vec<String>> {
    let chars: Vec<char> = input.chars().collect();
    let mut words = Vec::new();
    let mut word = String::new();
    let mut in_quotes = false;
    let mut word_started = false;
    let mut index = 0;

    while index < chars.len() {
        let ch = chars[index];

        if ch.is_whitespace() && !in_quotes {
            if word_started {
                words.push(std::mem::take(&mut word));
                word_started = false;
            }
            index += 1;
            continue;
        }

        if ch == '\\' {
            let start = index;
            while index < chars.len() && chars[index] == '\\' {
                index += 1;
            }
            let slash_count = index - start;

            if index < chars.len() && chars[index] == '"' {
                word.extend(std::iter::repeat_n('\\', slash_count / 2));
                if slash_count % 2 == 0 {
                    in_quotes = !in_quotes;
                } else {
                    word.push('"');
                }
                word_started = true;
                index += 1;
            } else {
                word.extend(std::iter::repeat_n('\\', slash_count));
                word_started = true;
            }
            continue;
        }

        if ch == '"' {
            in_quotes = !in_quotes;
            word_started = true;
            index += 1;
            continue;
        }

        word.push(ch);
        word_started = true;
        index += 1;
    }

    if in_quotes {
        return Err(Error::Argv(
            "template argv không hợp lệ: thiếu dấu nháy kép đóng".into(),
        ));
    }
    if word_started {
        words.push(word);
    }

    Ok(words)
}

#[cfg(test)]
mod tests {
    use super::split_argv;

    #[test]
    #[cfg_attr(not(windows), allow(unused_variables))]
    fn parses_windows_executable_and_model_paths() {
        let command = r#"G:\Work\llama.cpp\build\bin\llama-server.exe -m "E:\Models\Qwen3.5-Hasutsubomi-9B-Q4_K_M-no-mtp.gguf" -ngl 99 -c 8192 --flash-attn on --tensor-split 7,3 -c 32768 --jinja"#;
        let argv = split_argv(command).expect("valid Windows argv");

        #[cfg(windows)]
        assert_eq!(
            argv,
            vec![
                r#"G:\Work\llama.cpp\build\bin\llama-server.exe"#,
                "-m",
                r#"E:\Models\Qwen3.5-Hasutsubomi-9B-Q4_K_M-no-mtp.gguf"#,
                "-ngl",
                "99",
                "-c",
                "8192",
                "--flash-attn",
                "on",
                "--tensor-split",
                "7,3",
                "-c",
                "32768",
                "--jinja",
            ]
        );
    }

    #[test]
    #[cfg_attr(not(windows), allow(unused_variables))]
    fn keeps_a_quoted_argument_as_one_token() {
        let argv =
            split_argv(r#"tool.exe -m "C:\Models\model file.gguf" --flag"#).expect("valid argv");

        #[cfg(windows)]
        assert_eq!(
            argv,
            vec!["tool.exe", "-m", r#"C:\Models\model file.gguf"#, "--flag"]
        );
    }
}
