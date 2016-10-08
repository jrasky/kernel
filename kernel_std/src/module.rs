use collections::Vec;

use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug)]
pub struct Port {
    pub ty: Uuid, // port type
    pub places: Vec<u64> // places to fill in the port address
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Dependency {
    pub on: Uuid, // text being depended on
    pub places: Vec<u64> // places to fill in the text address
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Header {
    pub id: Uuid, // unique ID for this text
    pub base: Option<u64>, // optional base address
    pub size: u64, // length of this text
    pub write: bool, // whether this text should be writeable
    pub execute: bool, // whether this text should be executable
    pub depends: Vec<Dependency> // dependencies in this text
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Text {
    pub id: Uuid, // unique ID for this text, should match the Header
    pub data: Vec<u8> // the data itself
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Module {
    pub magic: Uuid, // 0af979b7-02c3-4ca6-b354-b709bec81199
    pub id: Uuid, // unique ID for this module
    pub ports: Vec<Port>, // set of ports for this module
    pub headers: Vec<Header>, // headers provided by this module
    pub texts: Vec<Text> // texts provided by this module
}
