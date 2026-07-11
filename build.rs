fn main() {
    windows_reactor_setup::as_self_contained();

    #[cfg(windows)]
    {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/app-icon.ico");
        resource
            .compile()
            .expect("compile Windows application icon");
    }
}
