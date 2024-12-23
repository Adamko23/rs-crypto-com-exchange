use serde::{Serialize, Deserialize};
use serde_aux::prelude::deserialize_number_from_string;
use std::fmt;

// Main container of a candlestick
#[derive(Serialize,Deserialize, Debug)]
pub struct CandlestickResult {
    /// Just the instrument name
    pub instrument_name: String,

    /// Subscription name used to subscribe this event
    pub subscription: String,

    /// The time interval affected by this candlestick. The options are
    /// 1m : one minute
    /// 5m : five minutes
    /// 15m : 15 minutes
    /// 30m: 30 minutes
    /// 1h : one hour
    /// 4h : 4 hours
    /// 6h : 6 hours
    /// 12h : 12 hours
    /// 1D : one day
    /// 7D : one week
    /// 14D : two weeks
    /// 1M : one month
    pub interval: String,

    /// Actual candlestick information
    pub data: Vec<Candlestick>
}

/// Candlestick received from subscription
#[derive(Serialize, Deserialize, Debug)]
pub struct Candlestick {

    /// Open price
    #[serde(rename = "o", deserialize_with = "deserialize_number_from_string")]
    pub open: f32,
    
    /// Close price
    #[serde(rename = "c", deserialize_with = "deserialize_number_from_string")]
    pub close: f32,

    /// Highest price
    #[serde(rename = "h", deserialize_with = "deserialize_number_from_string")]
    pub high: f32,

    /// Lowest price
    #[serde(rename = "l", deserialize_with = "deserialize_number_from_string")]
    pub low: f32,

    /// Volume
    #[serde(rename = "v", deserialize_with = "deserialize_number_from_string")]
    pub volume: f32,

    #[serde(rename = "ut", deserialize_with = "deserialize_number_from_string")]
    pub update_time: u64,

    #[serde(rename = "t", deserialize_with = "deserialize_number_from_string")]
    pub start_time: u64,
}

#[derive(Serialize,Deserialize, Debug)]
pub enum TimeFrame {
    #[serde(rename = "1m")]
    OneMinute,

    #[serde(rename = "5m")]
    FiveMinutes,

    #[serde(rename = "15m")]
    FiteenMinutes,

    #[serde(rename = "30m")]
    ThirtyMinutes,

    #[serde(rename = "1h")]
    OneHour,

    #[serde(rename = "4h")]
    FourHours,

    #[serde(rename = "6h")]
    SixHours,

    #[serde(rename = "12h")]
    TwelveHours,

    #[serde(rename = "1D")]
    OneDay,
    
    #[serde(rename = "7D")]
    OneWeek,

    #[serde(rename = "14D")]
    TwoWeeks,

    #[serde(rename = "1M")]
    OneMonth
}

impl fmt::Display for TimeFrame {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            TimeFrame::OneMinute => write!(f, "1m"),
            TimeFrame::FiveMinutes => write!(f, "5m"),
            TimeFrame::FiteenMinutes => write!(f, "15m"),
            TimeFrame::ThirtyMinutes => write!(f, "30m"),
            TimeFrame::OneHour => write!(f, "1h"),
            TimeFrame::FourHours => write!(f, "4h"),
            TimeFrame::SixHours => write!(f, "6h"),
            TimeFrame::TwelveHours => write!(f, "12h"),
            TimeFrame::OneDay => write!(f, "1D"),
            TimeFrame::OneWeek => write!(f, "7D"),
            TimeFrame::TwoWeeks => write!(f, "14D"),
            TimeFrame::OneMonth => write!(f, "1M"),
        }
    }
}

pub fn candlestick(time_frame: TimeFrame, instrument_name: &str) -> String {
    format!("candlestick.{time_frame}.{instrument_name}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::from_str;

    #[test]
    fn check_structure() {
        let json = "{
            \"instrument_name\": \"ETH_CRO\",
            \"subscription\": \"candlestick.1m.ETH_CRO\",
            \"channel\": \"candlestick\",
            \"depth\":300,
            \"interval\": \"1m\",
            \"data\":[
              {
                \"o\": \"162.03\",
                \"c\": \"162.04\",
                \"h\": \"161.96\",
                \"l\": \"161.98\",
                \"v\": \"336.452694\",
                \"t\": 1589443241000,
                \"ut\": 1589443242000
              },
              {
                \"o\": \"163.03\",
                \"c\": \"163.04\",
                \"h\": \"162.96\",
                \"l\": \"162.98\",
                \"v\": \"336.452694\",
                \"t\": 1589443241000,
                \"ut\": 1589443242000
              }
              ]
          }";

        let candlestick_result = from_str::<CandlestickResult>(json).unwrap();
        assert_eq!(candlestick_result.instrument_name, "ETH_CRO");
        assert_eq!(candlestick_result.subscription, "candlestick.1m.ETH_CRO");
        assert_eq!(candlestick_result.interval, "1m");
        assert_eq!(candlestick_result.data.len(), 2);

        // The data
        let data = &candlestick_result.data[0];
        assert_eq!(data.open, 162.03);
        assert_eq!(data.close, 162.04);
        assert_eq!(data.high, 161.96);
        assert_eq!(data.low, 161.98);
        assert_eq!(data.volume, 336.452694);
        assert_eq!(data.start_time, 1589443241000);
        assert_eq!(data.update_time, 1589443242000);
        
    }
}