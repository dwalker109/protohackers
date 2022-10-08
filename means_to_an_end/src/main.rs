fn main() {
    println!("Hello, world!");
}

type MessageBytes = [u8; 9];

#[derive(Debug, PartialEq)]
struct Insert {
    timestamp: i32,
    price: i32,
}

impl From<&MessageBytes> for Insert {
    fn from(b: &MessageBytes) -> Self {
        Self {
            timestamp: i32::from_be_bytes(b[1..5].try_into().unwrap()),
            price: i32::from_be_bytes(b[5..].try_into().unwrap()),
        }
    }
}

#[derive(Debug, PartialEq)]
struct Query {
    mintime: i32,
    maxtime: i32,
}

impl From<&MessageBytes> for Query {
    fn from(b: &MessageBytes) -> Self {
        Self {
            mintime: i32::from_be_bytes(b[1..5].try_into().unwrap()),
            maxtime: i32::from_be_bytes(b[5..].try_into().unwrap()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static INSERT_BYTES: MessageBytes = [0x49, 0x00, 0x00, 0x30, 0x39, 0x00, 0x00, 0x00, 0x65];
    static QUERY_BYTES: MessageBytes = [0x51, 0x00, 0x00, 0x03, 0xe8, 0x00, 0x01, 0x86, 0xa0];

    #[test]
    fn make_insert_from_bytes() {
        let msg = Insert::from(&INSERT_BYTES);
        assert_eq!(
            msg,
            Insert {
                timestamp: 12345,
                price: 101
            }
        );
    }

    #[test]
    fn make_query_from_bytes() {
        let msg = Query::from(&QUERY_BYTES);
        assert_eq!(
            msg,
            Query {
                mintime: 1000,
                maxtime: 100000
            }
        );
    }
}
