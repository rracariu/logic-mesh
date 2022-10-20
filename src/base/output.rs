pub trait Output {}

#[derive(Debug)]
pub struct BaseOutput<Link> {
    pub links: Vec<Link>,
}
