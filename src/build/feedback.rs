use colored::*;

pub struct FeedbackAnalyzer;

impl FeedbackAnalyzer {
    pub fn analyze(output: &str) -> Option<String> {
        // 1. Main function missing (Specific Linker Error)
        if output.contains("undefined reference to `main'")
            || output.contains("entry point must be defined")
        {
            return Some(format!(
                "Your project is missing a {} function.\nEnsure you have a valid entry point or set {} if this is a library.",
                "main()".bold().yellow(),
                "bin = \"lib\"".bold().green()
            ));
        }

        // 2. Generic Missing Library (Linker Error)
        if output.contains("LNK2019") || output.contains("undefined reference to") {
            return Some(format!(
                "It looks like a {} error.\nYou might be missing a library in {}.\nTry using {} to find the correct package.",
                "Linker".bold().red(),
                "cx.toml".bold().yellow(),
                "cx search".bold().green()
            ));
        }

        // 3. Missing Header (Compiler Error)
        if output.contains("fatal error: ") && output.contains("No such file or directory")
            || output.contains("cannot open include file")
        {
            // Extract the missing file name if possible?
            // Regex is heavy, let's just give general advice for now.
            return Some(format!(
                "It looks like a {} error.\nYou might be missing an include path or a dependency.\nCheck your {} dependencies or {} in cx.toml.",
                "Missing Header".bold().red(),
                "[dependencies]".bold().yellow(),
                "cflags".bold().yellow()
            ));
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linker_error() {
        let err = "error LNK2019: unresolved external symbol foo";
        let msg = FeedbackAnalyzer::analyze(err).unwrap();
        assert!(msg.contains("Linker error"));
        assert!(msg.contains("cx.toml"));
    }

    #[test]
    fn test_include_error() {
        let err = "fatal error: foo.h: No such file or directory";
        let msg = FeedbackAnalyzer::analyze(err).unwrap();
        assert!(msg.contains("Missing Header"));
    }

    #[test]
    fn test_main_error() {
        let err = "undefined reference to `main'";
        let msg = FeedbackAnalyzer::analyze(err).unwrap();
        assert!(msg.contains("missing a main() function"));
    }
}
