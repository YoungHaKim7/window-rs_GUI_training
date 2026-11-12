use windows::{
    Data::Xml::Dom::XmlDocument,
    Win32::{
        Foundation::CloseHandle,
        System::Threading::{CreateEventW, SetEvent, WaitForSingleObject},
        UI::WindowsAndMessaging::{MB_OK, MessageBoxA, MessageBoxW},
    },
    core::{Result, h, s, w},
};

fn main() -> Result<()> {
    let doc = XmlDocument::new()?;
    doc.LoadXml(h!("<html>hello world</html>"))?;

    let root = doc.DocumentElement()?;
    assert!(root.NodeName()? == "html");
    assert!(root.InnerText()? == "hello world");

    unsafe {
        let event = CreateEventW(None, true, false, None)?;
        SetEvent(event)?;
        WaitForSingleObject(event, 0);
        CloseHandle(event)?;

        MessageBoxA(None, s!("Ansi"), s!("Caption"), MB_OK);
        MessageBoxW(None, w!("Wide"), w!("Caption"), MB_OK);
    }

    Ok(())
}
