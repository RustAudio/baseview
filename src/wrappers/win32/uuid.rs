use std::fmt::{Display, Formatter};
use windows_sys::core::GUID;
use windows_sys::Win32::System::Rpc::UuidCreate;

pub struct Uuid(GUID);

impl Uuid {
    pub fn new() -> Self {
        let mut guid = GUID::default();

        // SAFETY: the passed pointer is valid, it comes from a mut reference
        unsafe { UuidCreate(&mut guid) };

        Self(guid)
    }
}

impl Display for Uuid {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:0X}-{:0X}-{:0X}-{:0X}{:0X}-{:0X}{:0X}{:0X}{:0X}{:0X}{:0X}\0",
            self.0.data1,
            self.0.data2,
            self.0.data3,
            self.0.data4[0],
            self.0.data4[1],
            self.0.data4[2],
            self.0.data4[3],
            self.0.data4[4],
            self.0.data4[5],
            self.0.data4[6],
            self.0.data4[7]
        )
    }
}
