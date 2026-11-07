use windows_sys::{
    Win32::Foundation::*, Win32::System::Threading::*, Win32::UI::WindowsAndMessaging::*, core::*,
};

fn main() {
    unsafe {
        let event = CreateEventW(std::ptr::null(), 1, 0, std::ptr::null());
        SetEvent(event);
        WaitForSingleObject(event, 0);
        CloseHandle(event);

        MessageBoxA(0 as _, s!("Ansi"), s!("Caption"), MB_OK);
        MessageBoxW(0 as _, w!("Wide"), w!("Caption"), MB_OK);
    }
}
