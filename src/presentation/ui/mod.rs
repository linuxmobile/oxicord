//! UI screens.

mod app;
pub mod backend;
mod chat_screen;
mod login_screen;
mod main_screen;
pub mod notification_popup;
pub mod quick_switcher;
pub mod splash_screen;
pub mod utils;

pub use app::{App, AppConfig};

pub use backend::{Action, Backend, BackendCommand};
pub use chat_screen::{ChatFocus, ChatKeyResult, ChatScreen, ChatScreenState};
pub use login_screen::{LoginAction, LoginScreen};
pub use main_screen::MainScreen;
pub use splash_screen::SplashScreen;
