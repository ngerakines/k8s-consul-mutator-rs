use std::env;

use derive_builder::Builder;

#[derive(Builder, Clone, Debug)]
#[builder(setter(into, strip_option))]
pub struct Settings {
    #[builder(setter(into), default = "self.default_version()")]
    pub version: String,

    #[builder(setter(into), default = "self.default_port()")]
    pub port: u16,

    #[builder(setter(into), default = "self.default_secure_port()")]
    pub secure_port: u16,

    #[builder(setter(into), default = "self.default_certificate()")]
    pub certificate: String,

    #[builder(setter(into), default = "self.default_certificate_key()")]
    pub certificate_key: String,

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

    #[builder(setter(into), default = "self.default_checksum_type()")]
    pub checksum_type: String,

    #[builder(setter(into), default = "self.default_key_manager_type()")]
    pub key_manager_type: String,

    /*
     * `SET_DEPLOYMENT_ANNOTATIONS` - Adds the checksum annotations to deployments if set to true. Default true.
     * `SET_DEPLOYMENT_SPEC_ANNOTATIONS` - Adds the checksum annotations to deployment specs if set to true. Default true.
     * `SET_DEPLOYMENT_TIMESTAMP` - Adds the `last-updated` annotation to deployments if set to true. Default true.
     * `SET_DEPLOYMENT_SPEC_TIMESTAMP` - Adds the `last-updated` annotation to deployment specs if set to true. Default false.
     */
    #[builder(setter(into), default = "self.default_set_deployment_annotations()")]
    pub set_deployment_annotations: bool,

    #[builder(
        setter(into),
        default = "self.default_set_deployment_spec_annotations()"
    )]
    pub set_deployment_spec_annotations: bool,

    #[builder(setter(into), default = "self.default_set_deployment_timestamp()")]
    pub set_deployment_timestamp: bool,

    #[builder(setter(into), default = "self.default_set_deployment_spec_timestamp()")]
    pub set_deployment_spec_timestamp: bool,
}

impl SettingsBuilder {
    fn default_version(&self) -> String {
        option_env!("GIT_HASH")
            .unwrap_or(env!("CARGO_PKG_VERSION", "develop"))
            .to_string()
    }

    fn default_port(&self) -> u16 {
        env::var("PORT")
            .unwrap_or("8080".to_string())
            .parse::<u16>()
            .unwrap_or(8080)
    }

    fn default_secure_port(&self) -> u16 {
        env::var("SECURE_PORT")
            .unwrap_or("8443".to_string())
            .parse::<u16>()
            .unwrap_or(8443)
    }

    fn default_update_debounce(&self) -> u16 {
        env::var("UPDATE_DEBOUNCE")
            .unwrap_or("60".to_string())
            .parse::<u16>()
            .unwrap_or(60)
    }

    fn default_certificate(&self) -> String {
        env::var("CERTIFICATE").unwrap_or("".to_string())
    }

    fn default_certificate_key(&self) -> String {
        env::var("CERTIFICATE_KEY").unwrap_or("".to_string())
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

    fn default_checksum_type(&self) -> String {
        env::var("CHECKSUM_TYPE")
            .unwrap_or("md5".to_string())
            .to_lowercase()
    }

    fn default_key_manager_type(&self) -> String {
        env::var("KEY_MANAGER_TYPE")
            .unwrap_or("memory".to_string())
            .to_lowercase()
    }

    fn default_set_deployment_annotations(&self) -> bool {
        let value = env::var("SET_DEPLOYMENT_ANNOTATIONS")
            .unwrap_or("true".into())
            .to_lowercase();
        match value.as_str() {
            "true" => true,
            "false" => false,
            _ => true,
        }
    }

    fn default_set_deployment_spec_annotations(&self) -> bool {
        let value = env::var("SET_DEPLOYMENT_SPEC_ANNOTATIONS")
            .unwrap_or("true".into())
            .to_lowercase();
        match value.as_str() {
            "true" => true,
            "false" => false,
            _ => true,
        }
    }

    fn default_set_deployment_timestamp(&self) -> bool {
        let value = env::var("SET_DEPLOYMENT_TIMESTAMP")
            .unwrap_or("true".into())
            .to_lowercase();
        match value.as_str() {
            "true" => true,
            "false" => false,
            _ => true,
        }
    }

    fn default_set_deployment_spec_timestamp(&self) -> bool {
        let value = env::var("SET_DEPLOYMENT_SPEC_TIMESTAMP")
            .unwrap_or("false".into())
            .to_lowercase();
        match value.as_str() {
            "true" => true,
            "false" => false,
            _ => false,
        }
    }
}

impl Settings {
    pub fn is_insecure_enabled(&self) -> bool {
        self.port != 0
    }

    pub fn is_secure_enabled(&self) -> bool {
        self.secure_port != 0 && !self.certificate.is_empty() && !self.certificate_key.is_empty()
    }
}
