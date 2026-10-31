fn main() -> iced::Result {
    // SAFETY: 在 iced 启动多线程运行时之前（单线程阶段）写入，且不覆盖用户显式设置。
    if std::env::var_os("WGPU_POWER_PREF").is_none() {
        unsafe { std::env::set_var("WGPU_POWER_PREF", "low") };
    }
    opencode_session_manager::app::run()
}
