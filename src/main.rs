use std::{io, time};
use std::fmt;

use rustdds::*;
use serde::{Serialize, Deserialize};

use log4rs::{
  append::console::ConsoleAppender,
  config::{Appender, Root},
  Config,
};
use log::LevelFilter;
use futures::StreamExt;
use smol::Timer;

use log::{debug, error, info, trace, warn};

use structopt::StructOpt;
use strum::VariantNames;

use std::iter::successors;

const SECOND: time::Duration = time::Duration::from_millis(1000);

const ONES: [&str; 20] = [
    "zero",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
];
const TENS: [&str; 10] = [
    "zero", "ten", "twenty", "thirty", "forty", "fifty", "sixty",
    "seventy", "eighty", "ninety",
];
const ORDERS: [&str; 7] = [
    "zero",
    "thousand",
    "million",
    "billion",
    "trillion",
    "quadrillion",
    "quintillion", // enough for u64::MAX
];

// #[derive(Debug)]
// struct Structure(i32);
//   
// #[derive(Debug)]
// struct Deep(Structure);
//   
// #[derive(Debug, StructOpt)]
// struct Args {
//     #[structopt(long, possible_values = Enumerated::VARIANTS)]
//     enumerated: Enumerated,
// } 

#[derive(Debug, StructOpt)]
struct Args {
    #[structopt(long, possible_values = ServerType::VARIANTS)]
    servertype: ServerType,
}

#[derive(Debug, strum::EnumString, strum::EnumVariantNames)]
#[strum(serialize_all = "kebab-case")]
enum ServerType {
    Pub,
    Sub,
}


impl fmt::Display for ServerType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ServerType::Pub => write!(f, "pub"),
            ServerType::Sub => write!(f, "sub"),
        }
    }
}

fn main() {
  println!("== Starting Rust DDS PubSub");
  
  configure_logging();
  
  let args = Args::from_args();
  debug!("{:?}", args);
  debug!("{:?}", args.servertype);
    
  let arg = args.servertype;
  debug!("{:?}", arg);
    
  let server_type = arg.to_string();   
  info!("server_type = {:?}", server_type);
  
//   println!("Structure: {:?}", Structure(3));
//   info!("Deep: {:?}", Deep(Structure(0)));
//   debug!("Deep: {:?}", Deep(Structure(7)));
//   trace!("Deep: {:?}", Deep(Structure(77)));
//   warn!("Deep: {:?}", Deep(Structure(999)));    
//   error!("Deep: {:?}", Deep(Structure(666)));

  // Create a DDS DomainParticipant
  let domain_participant = DomainParticipant::new(0).unwrap();
  info!("Created the DomainParticipant");
  
  // Create a DDS Quality of Service
  let qos = QosPolicyBuilder::new()
    .reliability(policy::Reliability::Reliable {max_blocking_time: rustdds::Duration::ZERO})
    .build();
  info!("Created the Quality of Service");
  
  // Create a DDS Subscriber 
  let subscriber = domain_participant.create_subscriber(&qos).unwrap();
  info!("Created the DDS Subscriber");
  
  // Create a DDS Publisher
  let publisher = domain_participant.create_publisher(&qos).unwrap();
  info!("Created the DDS Publisher");
  
  // Create the DDS Topic
  let some_topic = domain_participant.create_topic("some_topic".to_string(), "SomeType".to_string(), &qos, TopicKind::NoKey).unwrap();
  info!("Created the DDS Topic");
  
  #[derive(Serialize, Deserialize, Debug)]
  struct SomeType {
    a: i32,
    b: u32,
    c: String,
  }
  
  // ---
   
  if "sub" == server_type {
  
    let reader = subscriber
    .create_datareader_no_key::<SomeType, CDRDeserializerAdapter<SomeType>>(
      &some_topic,
      None)
    .unwrap();
  info!("Created the DDS DataReader");
  
    info!("-- Start Subscriber message listening --");
    
    smol::block_on(async {
      let mut datareader_stream = reader.async_sample_stream();
      let mut datareader_event_stream = datareader_stream.async_event_stream();

      loop {
        futures::select! {
          r=datareader_stream.select_next_some()=>{
            match r{
              Ok(d)=>{println!("{} {} {}",d.a, d.b, d.c)},
              Err(e)=> {
                println!("{:?}", e);
                break;
              }
            }
          }
          e=datareader_event_stream.select_next_some()=>{
            println!("DataReader event: {e:?}");
          }
        }
      }
    })    
    
  } else if "pub" == server_type {
  
    let  writer = publisher
    .create_datawriter_no_key::<SomeType, CDRSerializerAdapter<SomeType>> (
      &some_topic,
      None)
    .unwrap();
  info!("Created the DDS DataWriter");
  
    info!("-- Start Publisher message sending --");
    
    smol::block_on(async {
      let mut tick_stream = futures::StreamExt::fuse(Timer::interval(SECOND));

      let mut datawriter_event_stream = writer.as_async_status_stream();

      let mut i_i32 = 0;
      let mut i_u32: u32 = 0;
      
      loop {
        futures::select! {
          _= tick_stream.select_next_some()=>{
            let some_data = SomeType { 
            a: i_i32 ,
            b: i_u32 * 10,
            c: encode(i_u32.try_into().unwrap()), //i.to_string(),
            };
            i_i32 += 1;
            i_u32 += 1;
            writer.async_write(some_data,None).await.unwrap_or_else(|e| println!("DataWriter write failed: {e:?}"));
            println!("Sent message {}", i_i32 -1);
          }
          e= datawriter_event_stream.select_next_some()=>{
            println!("DataWriter event: {e:?}");
          }
        }
      }
    })

  } else {
    error!("Invalid argument: {:?}", server_type);
    return;
  }
  
}

fn configure_logging() {
  println!("Enter: configure_logging");
  
  log4rs::init_file(
    "logging-config.yaml",
    log4rs::config::Deserializers::default(),
  )
  .unwrap_or_else(|e| {
    match e.downcast_ref::<io::Error>() {
      Some(os_err) if os_err.kind() == io::ErrorKind::NotFound => {
        println!("No logging-config.yaml file found.");
        let stdout = ConsoleAppender::builder().build();
        let conf = Config::builder()
          .appender(Appender::builder().build("stdout", Box::new(stdout)))
          .build(Root::builder().appender("stdout").build(LevelFilter::Error))
          .unwrap();
        log4rs::init_config(conf).unwrap();
      }
      other_error => panic!("Logging config problem {other_error:?}"),
    }
  });
}

// See: https://stackoverflow.com/questions/61603849/how-to-transform-a-rust-number-into-english-words-like-1-one
pub fn encode(num: u32) -> String {
    match num {
        0..=19 => ONES[num as usize].to_string(),
        20..=99 => {
            let upper = (num / 10) as usize;
            match num % 10 {
                0 => TENS[upper].to_string(),
                lower => format!("{}-{}", TENS[upper], encode(lower)),
            }
        }
        100..=999 => format_num(num, 100, "hundred"),
        _ => {
            let (div, order) =
                successors(Some(1u32), |v| v.checked_mul(1000))
                    .zip(ORDERS.iter())
                    .find(|&(e, _)| e > num / 1000)
                    .unwrap();

            format_num(num, div, order)
        }
    }
}

fn format_num(num: u32, div: u32, order: &str) -> String {
    match (num / div, num % div) {
        (upper, 0) => format!("{} {}", encode(upper), order),
        (upper, lower) => {
            format!("{} {} {}", encode(upper), order, encode(lower))
        }
    }
}