use {serde::Serialize, zbus::zvariant::Type};

const PORTAL_SUCCESS: u32 = 0;
const PORTAL_CANCELLED: u32 = 1;

#[derive(Serialize, Type)]
pub struct Response<T: Type>(u32, T);

impl<T: Type> Response<T> {
    pub fn success(t: T) -> Self {
        Self(PORTAL_SUCCESS, t)
    }

    pub fn cancelled() -> Self
    where
        T: Default,
    {
        Self(PORTAL_CANCELLED, T::default())
    }
}
