use std::env;

use derive_builder::Builder;

#[derive(Builder, Clone, Debug)]
#[builder(setter(into, strip_option))]
pub struct Settings {
    #[builder(setter(into), default = "self.default_update_debounce()")]
    pub update_debounce: u16,

    #[builder(
        setter(into),
        default = "self.default_watch_dispatcher_first_reconcile()"
    )]
    pub watch_dispatcher_first_reconcile: u16,

    #[builder(setter(into), default = "self.default_watch_dispatcher_reconcile()")]
    pub watch_dispatcher_reconcile: u16,

    #[builder(setter(into), default = "self.default_watch_dispatcher_debounce()")]
    pub watch_dispatcher_debounce: u16,

    #[builder(setter(into), default = "self.default_check_key_timeout()")]
    pub check_key_timeout: String,

    #[builder(setter(into), default = "self.default_check_key_idle()")]
    pub check_key_idle: u16,

    #[builder(setter(into), default = "self.default_check_key_error_wait()")]
    pub check_key_error_wait: u16,
}

impl SettingsBuilder {
    fn default_update_debounce(&self) -> u16 {
        env::var("UPDATE_DEBOUNCE")
            .unwrap_or("60".to_string())
            .parse::<u16>()
            .unwrap_or(60)
    }

    fn default_watch_dispatcher_first_reconcile(&self) -> u16 {
        env::var("WATCH_DISPATCHER_FIRST_RECONCILE")
            .unwrap_or("30".to_string())
            .parse::<u16>()
            .unwrap_or(30)
    }

    fn default_watch_dispatcher_reconcile(&self) -> u16 {
        env::var("WATCH_DISPATCHER_RECONCILE")
            .unwrap_or("1800".to_string())
            .parse::<u16>()
            .unwrap_or(1800)
    }

    fn default_watch_dispatcher_debounce(&self) -> u16 {
        env::var("WATCH_DISPATCHER_DEBOUNCE")
            .unwrap_or("60".to_string())
            .parse::<u16>()
            .unwrap_or(60)
    }

    fn default_check_key_timeout(&self) -> String {
        env::var("CHECK_KEY_TIMEOUT").unwrap_or("10s".to_string())
    }

    fn default_check_key_idle(&self) -> u16 {
        env::var("CHECK_KEY_IDLE")
            .unwrap_or("60".to_string())
            .parse::<u16>()
            .unwrap_or(60)
    }

    fn default_check_key_error_wait(&self) -> u16 {
        env::var("CHECK_KEY_ERROR_WAIT")
            .unwrap_or("60".to_string())
            .parse::<u16>()
            .unwrap_or(60)
    }
}
