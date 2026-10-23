//! Commands the UI sends to the app, as `https://arbalette.ui/command/<name>` requests.

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Command {
    /// The UI is listening: send it the current state.
    Ready,
    Back,
    Forward,
    Reload,
}

impl Command {
    pub fn parse(path: &str) -> Option<Self> {
        match path {
            "ready" => Some(Self::Ready),
            "back" => Some(Self::Back),
            "forward" => Some(Self::Forward),
            "reload" => Some(Self::Reload),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Command;

    #[test]
    fn parses_known_commands() {
        assert_eq!(Command::parse("ready"), Some(Command::Ready));
        assert_eq!(Command::parse("back"), Some(Command::Back));
        assert_eq!(Command::parse("forward"), Some(Command::Forward));
        assert_eq!(Command::parse("reload"), Some(Command::Reload));
    }

    #[test]
    fn ignores_unknown_commands() {
        assert_eq!(Command::parse("quit"), None);
    }
}
