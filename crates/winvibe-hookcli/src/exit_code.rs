/// hookcli 进程退出码
#[repr(i32)]
pub enum ExitCode {
    /// 协议成功（已写 stdout JSON）
    ProtocolSuccess = 0,
    /// fail-closed（安全降级，阻止工具执行）
    FailClosed = 2,
    /// 配置错误
    ConfigError = 78,
}

impl ExitCode {
    /// 以此退出码终止进程
    pub fn exit(self) -> ! {
        std::process::exit(self as i32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_success_is_0() {
        assert_eq!(ExitCode::ProtocolSuccess as i32, 0);
    }

    #[test]
    fn fail_closed_is_2() {
        assert_eq!(ExitCode::FailClosed as i32, 2);
    }

    #[test]
    fn config_error_is_78() {
        assert_eq!(ExitCode::ConfigError as i32, 78);
    }
}
