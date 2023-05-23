use std::{io::{Cursor, Write, Read}, net::{SocketAddr, IpAddr}, time::{Instant, Duration}};
use byteorder::{WriteBytesExt, BigEndian, ReadBytesExt};
use crate::err::TResult;

pub trait Serializable {
    type Output;

    fn write(&self, buf: &mut Vec<u8>) -> TResult;
    fn read(buf: &mut Cursor<&[u8]>) -> TResult<Self::Output>;

    fn size(&self) -> usize;
}

pub fn write_byte_arr<const N: usize>(buf: &mut Vec<u8>, arr: &[u8; N]) -> TResult {
    Ok(buf.write_all(arr)?)
}

pub fn read_byte_arr<const N: usize>(buf: &mut Cursor<&[u8]>) -> TResult<[u8; N]> {
    let mut arr = [0; N];
    buf.read_exact(&mut arr)?;
    Ok(arr)
}

pub fn write_arr<T: Serializable, const N: usize>(buf: &mut Vec<u8>, arr: &[T; N]) -> TResult {
    //buf.write_u16::<BigEndian>(N as u16)?;
    Ok(for t in arr.iter() {
        t.write(buf)?;
    })
}

pub fn read_arr<T: Serializable<Output = T> + Copy + Default, const N: usize>(buf: &mut Cursor<&[u8]>) -> TResult<[T; N]> {
    let mut arr = [T::default(); N];
    for i in 0..N {
        arr[i] = T::read(buf)?;
    }
    Ok(arr)
}

pub fn write_byte_vec(buf: &mut Vec<u8>, vec: &Vec<u8>) -> TResult {
    buf.write_u16::<BigEndian>(vec.len() as u16)?;
    Ok(buf.write_all(vec)?)
}

pub fn read_byte_vec(buf: &mut Cursor<&[u8]>) -> TResult<Vec<u8>> {
    let len = buf.read_u16::<BigEndian>()?;
    let mut vec = Vec::with_capacity(len as usize);
    buf.read_exact(&mut vec)?;
    Ok(vec)
}

pub fn write_vec<T: Serializable>(buf: &mut Vec<u8>, vec: &Vec<T>) -> TResult {
    buf.write_u32::<BigEndian>(vec.len() as u32)?;
    for i in vec.iter() {
        i.write(buf)?;
    }
    Ok(())
}

pub fn read_vec<T: Serializable<Output = T>>(buf: &mut Cursor<&[u8]>) -> TResult<Vec<T>> {
    let len = buf.read_u32::<BigEndian>()? as usize;
    let mut vec = vec![];
    for _ in 0..len {
        vec.push(T::read(buf)?);
    }
    Ok(vec)
}

pub fn write_string(buf: &mut Vec<u8>, str: &String) -> TResult {
    write_byte_vec(buf, &str.as_bytes().to_vec())
}

pub fn read_string(buf: &mut Cursor<&[u8]>) -> TResult<String> {
    Ok(String::from_utf8(read_byte_vec(buf)?).unwrap()) // TODO: Check err...
}

pub fn write_sock_addr(buf: &mut Vec<u8>, addr: &SocketAddr) -> TResult {
    match addr.ip() {
        std::net::IpAddr::V4(ip) => {
            buf.write_u8(0)?;
            write_byte_arr::<4>(buf, &ip.octets())
        },
        std::net::IpAddr::V6(ip) =>  {
            buf.write_u8(1)?;
            write_byte_arr::<16>(buf, &ip.octets())
        }
    }?;
    buf.write_u16::<BigEndian>(addr.port())?;
    Ok(())
}

pub fn read_sock_addr(buf: &mut Cursor<&[u8]>) -> TResult<SocketAddr> {
    let ip_addr_type = buf.read_u8()?;
    let ip_addr = match ip_addr_type {
        0 => IpAddr::V4(read_byte_arr::<4>(buf)?.into()),
        1 => IpAddr::V6(read_byte_arr::<16>(buf)?.into()),
        n @ _ => panic!("Index out of bounds: {n}.")
    };    
    let port = buf.read_u16::<BigEndian>()?;
    Ok((ip_addr, port).into())
}

pub fn get_sock_addr_size(addr: &SocketAddr) -> usize {
    (if addr.is_ipv4() {
        4
    } else {
        16
    }) + 2
}

pub fn write_instant(buf: &mut Vec<u8>, time: Instant) -> TResult {
    Ok(buf.write_f32::<BigEndian>(time.elapsed().as_secs_f32())?)
}

pub fn read_instant(buf: &mut Cursor<&[u8]>) -> TResult<Instant> {
    // TODO: Improve serialization
    let elapsed = Duration::from_secs_f32(buf.read_f32::<BigEndian>()?);
    Ok(Instant::now().checked_sub(elapsed).unwrap())
}

pub trait Serializer : Serializable {
    fn serialize(&self) -> TResult<Vec<u8>> {
        let mut buf = vec![];
        self.write(&mut buf)?;
        Ok(buf)
    }
    fn deserialize(buf: &[u8]) -> TResult<Self::Output> {
        Ok(Self::read(&mut Cursor::new(&buf))?)
    }
}
