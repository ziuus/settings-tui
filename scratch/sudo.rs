pub enum SudoAction {
    ToggleService(String, bool),
    SetHostname(String),
}
