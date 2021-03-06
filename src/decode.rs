//! Decode NBT values from files or other readable sources.
use std;

use super::{Error, Result, Tag, TagType, ListData, CompoundData, Decodable, Compression};

use std::fs::File;
use std::io::Read;
use std::collections::HashMap;

use flate2::read::GzDecoder;


fn read_string<R: Read>(reader: &mut R) -> Result<String> {
    let mut raw_name_len = [0_u8; 2];
    try!(reader.read(&mut raw_name_len));

    let name_len = i16::from_bytes(&raw_name_len).unwrap() as usize;

    Ok(if name_len > 0 {
        let mut raw_name_dat = vec![0; name_len].into_boxed_slice();
        try!(reader.read(&mut *raw_name_dat));

        String::from_utf8_lossy(&*raw_name_dat).into_owned()
    } else {
        "".to_owned()
    })
}

fn read_primitive<R: Read, T: Decodable>(reader: &mut R) -> Result<T> {
    let siz = unsafe { std::intrinsics::size_of::<T>() };

    let mut slice = vec![0; siz].into_boxed_slice();
    try!(reader.read(&mut *slice));

    match T::from_bytes(&slice) {
        Some(x) => Ok(x),
        None    => Err(Error::Malformed),
    }
}


fn read_value<R: Read>(reader: &mut R, vtype: TagType) -> Result<Tag> {
    match vtype {
        // Can't read the end marker as an actual tag
        TagType::End   => Err(Error::Malformed),

        TagType::Byte   => Ok(Tag::Byte(  try!(read_primitive(reader)))),
        TagType::Short  => Ok(Tag::Short( try!(read_primitive(reader)))),
        TagType::Int    => Ok(Tag::Int(   try!(read_primitive(reader)))),
        TagType::Long   => Ok(Tag::Long(  try!(read_primitive(reader)))),
        TagType::Float  => Ok(Tag::Float( try!(read_primitive(reader)))),
        TagType::Double => Ok(Tag::Double(try!(read_primitive(reader)))),

        TagType::ByteArray => {
            let len = try!(read_primitive::<_, i32>(reader));
            let mut bytes = vec![0_u8; len as usize];

            try!(reader.read(bytes.as_mut_slice()));

            Ok(Tag::ByteArray(bytes))
        },

        TagType::String => Ok(Tag::String(try!(read_string(reader)))),

        TagType::List => {
            let et = try!(read_primitive::<_, i8>(reader));
            let tt = TagType::from_binary(et as u8);
            let len = try!(read_primitive::<_, i32>(reader));

            if tt.is_none() && et != 0 {
                return Err(Error::Malformed);

            }

            let mut vec = Vec::with_capacity(len as usize);

            for _ in 0 .. len {
                vec.push(try!(read_value(reader, tt.unwrap())));
            }

            Ok(Tag::List(ListData {
                element_type: TagType::from_binary(et as u8).unwrap(),
                elements: vec
            }))
        },

        TagType::Compound => {
            let mut map = HashMap::new();

            loop {
                match read_tag(reader) {
                    Ok((_, Tag::End)) => break,
                    Ok((n, v))        => map.insert(n, v),
                    Err(e)            => return Err(e)
                };
            }

            Ok(Tag::Compound(CompoundData { elements: map }))
        },

        TagType::IntArray => {
            let len = try!(read_primitive::<_, i32>(reader));
            let mut ints = Vec::with_capacity(len as usize);

            for _ in 0 .. len {
                ints.push(try!(read_primitive::<_, i32>(reader)));
            }

            Ok(Tag::IntArray(ints))
        }
    }
}

fn read_tag<R: Read>(reader: &mut R) -> Result<(String, Tag)> {
    let mut header = [0_u8; 1];
    try!(reader.read(&mut header));

    if let Some(t) = TagType::from_binary(header[0]) {
        if t == TagType::End {
            return Ok(("".to_owned(), Tag::End));
        }

        let name = try!(read_string(reader));
        let v = try!(read_value(reader, t));

        Ok((name, v))

    } else {
        return Err(Error::Malformed)
    }

}

/// Decode NBT tags.
pub struct Decoder {
    reader: Box<Read>
}

impl Decoder {
    /// Create a new Decoder from an existing reader that will be taken
    /// ownership over.
    pub fn from_reader<R: Read + 'static>(reader: R) -> Decoder {
        Decoder {
            reader: Box::new(reader)
        }
    }

    pub fn from_file(file: &str, c: Compression) -> Result<Decoder> {
        Ok(Decoder {
            reader: match c {
                Compression::Uncompressed => Box::new(try!(File::open(file))),
                Compression::GZip =>
                    Box::new(try!(GzDecoder::new(try!(File::open(file)))))
            }
        })
    }

    /// Read a named tag from the stream.
    pub fn read_tag(&mut self) -> Result<(String, Tag)> {
        read_tag(&mut self.reader)
    }
}

