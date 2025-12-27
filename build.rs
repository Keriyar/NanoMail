fn main() {
    // 编译 Slint UI
    slint_build::compile("ui/main.slint").unwrap();

    // Windows 平台:嵌入应用图标
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/icons/NanoMail.ico"); // 设置应用图标
        res.compile().unwrap();
    }
}
