#[derive(Debug, Clone, serde :: Deserialize, serde :: Serialize)]
#[doc = "A customer.\n\nContains the complete profile of a customer."]
pub struct Customer {
    #[doc = "Full name."]
    pub name: String,
    #[doc = "Customer ID."]
    pub id: i32,
    #[doc = "The customer's net worth in dollars."]
    pub net_worth: f64,
    #[doc = "Time the customer joined the site."]
    pub join_date: ::humblegen_rt::chrono::DateTime<::humblegen_rt::chrono::prelude::Utc>,
    #[doc = "Date of birth."]
    pub birthday: ::humblegen_rt::chrono::NaiveDate,
    #[doc = "Is the customer a VIP?"]
    pub is_vip: bool,
    #[doc = "Favorite color."]
    pub favorite_color: Color,
    #[doc = "Codenames, spy aliases for customer."]
    pub aliases: Vec<String>,
    #[doc = "Current location in one millionth of a degree lat/lon."]
    pub coords: (i32, i32),
    #[doc = "Primary email."]
    pub email: Option<String>,
    #[doc = "List of horses the customer backed in a race, including dollar amounts."]
    pub bets: ::std::collections::HashMap<String, f64>,
    #[doc = "The empty type is supported"]
    pub empty: (),
    #[doc = "The uuid type is supported"]
    pub unique_id: ::humblegen_rt::uuid::Uuid,
    #[doc = "The bytes type is supported"]
    #[serde(deserialize_with = "::humblegen_rt::serialization_helpers::deser_bytes")]
    #[serde(serialize_with = "::humblegen_rt::serialization_helpers::ser_bytes")]
    pub profile_pic: Vec<u8>,
}
#[derive(Debug, Clone, serde :: Deserialize, serde :: Serialize)]
#[doc = "A color."]
pub enum Color {
    #[doc = "Pure red."]
    Red,
    #[doc = "Pure blue."]
    Blue,
    #[doc = "Pure green."]
    Green,
    #[doc = "RGB Color."]
    Rgb(u8, u8, u8),
    #[doc = "Web-color name,"]
    Named(String),
    #[doc = "Hue, saturation, value color."]
    Hsv {
        #[doc = "Hue."]
        h: u8,
        #[doc = "Saturation."]
        s: u8,
        #[doc = "Value."]
        v: u8,
    },
}
