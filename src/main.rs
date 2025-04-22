use std::net::{SocketAddr, UdpSocket};
use std::net::TcpStream;
use std::time::Duration;

macro_rules! format_torrent_msg {
    ($file_name:expr) => {format!("GET {0}.torrent\n", $file_name)}
}

macro_rules! format_file_msg {
    ($file_name:expr, $block_num: expr) => (format!("GET {0}:{1}\n", $file_name, $block_num))
}


fn main() {
    let file_name = std::env::args().nth(1).expect("no file name");
    let addr = std::env::args().nth(2).expect("no sever addr") + ":" + &std::env::args().nth(3).expect("no port number");
        
    if let Some(i) = torrent_msg(&addr, &file_name){
        i.print();

    }else{
        println!("no data");
    }
}

struct TorrentResponse{
    num_blocks: usize,
    file_size: usize,
    addr1: String,
    addr2: String
}

impl TorrentResponse{
    fn print(&self){
        println!("num_blocks: {0}, file_size: {1}, addr1: {2}, addr2: {3}", self.num_blocks, self.file_size, self.addr1, self.addr2);
    }
}

fn update_ip_pool(){
    todo!();
}

fn torrent_msg(addr: &str, file_name: &str) -> Option<TorrentResponse>{
    let addrs = [
        SocketAddr::from(([127, 0, 0, 1], 3400)),
        SocketAddr::from(([127, 0, 0, 1], 3401)),
    ];

    let Ok(socket) = UdpSocket::bind(&addrs[..]) else{
        return None;
    };
    //sets tile out
    socket.set_read_timeout(Some(Duration::from_millis(1000)));


    //send init msg
    let Ok(_) = socket.send_to(format_torrent_msg!(file_name).as_bytes(), &addr) else{
        print!("failed to send");
        return None;
    };


    let mut buff = [0u8; 2048];

    //reads bs
    let Ok((amt, _)) = socket.recv_from(&mut buff) else{ return None; };

    let mut newlines : Vec<usize> = Vec::new();
    let mut cols : Vec<usize> = Vec::new();

    for (i, byte) in buff.iter().enumerate(){
        if byte == &('\n' as u8){
            newlines.push(i);
        }

        if byte == &(':' as u8){
            cols.push(i);
        }
    }

    if newlines.len() > 6{
        return None; //bad format data merp
    }
    
    let num_blocks = get_nums_in_line(&String::from_utf8(buff[cols[0] .. newlines[0]].to_vec()).expect("merp"));
    let file_size = get_nums_in_line(&String::from_utf8(buff[cols[1] .. newlines[1]].to_vec()).expect("merp"));
    let ip1 = String::from_utf8(buff[cols[2] + 1 .. newlines[2]].to_vec()).expect("merp");
    let ip1 = ip1.trim();
    let port1 = String::from_utf8(buff[cols[3] + 1.. newlines[3]].to_vec()).expect("merp");
    let port1 = port1.trim();
    let ip2 = String::from_utf8(buff[cols[4] + 1.. newlines[4]].to_vec()).expect("merp");
    let ip2 = ip2.trim();
    let port2 = String::from_utf8(buff[cols[5] + 1 .. newlines[5]].to_vec()).expect("merp");
    let port2 = port2.trim();

    Some(TorrentResponse{
        num_blocks: num_blocks,
        file_size: file_size,
        addr1: format!("{ip1}:{port1}"),
        addr2: format!("{ip2}:{port2}") 
    })


}

fn get_nums_in_line(line: &str) -> usize{
    let mut size: usize = 0;
    for c in line.chars(){
        if let Some(d) = c.to_digit(10){
            size *= 10;
            size += d as usize;
        }
    }
    size

}

struct File{
    blocks: Vec<Vec<u8>>,
    file_name: String,
    file_size: usize

}


impl File{
    fn new(num_blocks, file_name: &str, file_size: usize) -> Self{
        return Self{
            blocks: vec![vec![]; num_blocks],
            file_name: file_name.to_string(),
            file_size
        }

    }
    fn get_blocks(&mut self, addr: &str){
        todo!()
    }

    fn get_block(&mut self, addr: &str, block_num: usize){
        let mut stream = TcpStream::connect(addr).expect("merp");
        //if stream.is_err(){return;}
        //let mut stream = stream.unwrap();  

        //Step one send the get request
        let to_send = format!("GET {0}:{1}\n", self.file_name, block_num);
        stream.write(to_send.as_bytes()).expect("merp");

        //read in response
        let mut buf : Vec<u8> = vec![];
        let mut temp_buf = [0u8; 2400];

        let max_read_time = Duration::from_millis(5000);
        stream.set_read_timeout(Some(max_read_time)).expect("merp failed to set time out bs");

        loop{
            match stream.read(&mut temp_buf){
                Ok(i) => {
                    println!("here");
                    for byte_idx in 0..i{
                        buf.push(temp_buf[byte_idx]);
                    }

                    if i == 0 {println!("LEN 0 break out pal"); break;};
                }
                Err(_) => {
                    println!("hit an error reading the file");
                    break;
                }
            }
        }
    }

    fn write_file(&mut self, file_name: String){
        todo!()
    }
}

//grabs all blocks from the addr
