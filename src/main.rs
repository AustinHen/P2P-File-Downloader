use std::net::UdpSocket;
use std::sync::mpsc;
use std::thread;
use std::sync::{Arc, Mutex};
use std::io::prelude::*;
use std::fs::File;
use std::net::TcpStream;
use std::str;
use std::time::{Duration, SystemTime};
use std::collections::{HashSet, BinaryHeap, VecDeque};
use std::cmp::Ordering; use std::cmp::Reverse;

macro_rules! format_torrent_msg {
    ($file_name:expr) => {format!("GET {0}.torrent\n", $file_name)}
}

const BLOCKS_PER_ORDER : usize = 1;
const NUM_THREADS : usize = 5;

#[derive(Clone)]
struct Order{
    addr: String,
    blocks: Option<Vec<usize>>, //if empty order is just torrent msg
}

struct Block(Vec<u8>);

struct OrderResponse{
    thread_num: usize,
    elapsed: Duration,
    response: ResponseContent
}

enum ResponseContent{
    BlocksFilled(Vec<usize>),
    Torrent(Option<TorrentResponse>),
}

struct TorrentResponse{
    num_blocks: usize,
    file_size: usize,
    addr1: String,
    addr2: String
}

#[derive(Eq, PartialEq)]
struct Addr{
    address: String,
    dur: Duration,
}

impl Ord for Addr{
    fn cmp(&self, other: &Self) -> Ordering {
        self.dur.cmp(&other.dur)
    }
}

impl PartialOrd for Addr{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.dur.partial_cmp(&other.dur)
    }
}


fn main(){
    let file_name = std::env::args().nth(1).expect("no file name");
    let addr = std::env::args().nth(2).expect("no sever addr") + ":" + &std::env::args().nth(3).expect("no port number");
        
    let torrent = init_torrent_msg(&addr, &file_name);

    let mut known_addrs : VecDeque<String> = vec![torrent.addr1.clone(), torrent.addr2.clone(), addr.clone()].into(); 
    let mut addr_heap = BinaryHeap::new(); 
    let mut un_measured_addrs = VecDeque::new(); 
    un_measured_addrs.push_back(Addr{address: torrent.addr1.clone(), dur: Duration::MAX});
    un_measured_addrs.push_back(Addr{address: torrent.addr2.clone(), dur: Duration::MAX});

    let blocks: Arc<Vec<Mutex<Option<Block>>>> = Arc::new(
        (0..torrent.num_blocks).map(|_| Mutex::new(None)).collect()
    );

    //blocks that we dont have yet 
    let mut not_downloaded_blocks: HashSet<usize> = (0..torrent.num_blocks).map(|i| i).collect(); 
    //blocks that are not  trying to be downloaded by other threads
    let mut non_pending_blocks: VecDeque<usize> = (0..torrent.num_blocks).map(|i| i).collect(); 

    //set up threads
    let mut threads = Vec::new();
    let mut thread_orders : Vec<Option<Order>> = Vec::new(); //all orders currently happening
    let mut order_chans = Vec::new();
    let (res_sender, res_rcvr): (mpsc::Sender<OrderResponse>, mpsc::Receiver<OrderResponse>) = mpsc::channel();

    for i in 0..NUM_THREADS{
        let (order_sender, order_rcvr): (mpsc::Sender<Option<Order>>, mpsc::Receiver<Option<Order>>) = mpsc::channel();
        order_chans.push(order_sender);
        thread_orders.push(None);
        let sender = res_sender.clone();
        let thread_blocks = blocks.clone();
        let file_name = file_name.clone();
        threads.push(thread::spawn(move || {worker_thread(i, order_rcvr, sender, thread_blocks, &file_name);}));
    }


    //main loop
    while not_downloaded_blocks.len() > 0 {
        //sends out order
        for (i, order) in thread_orders.iter_mut().enumerate(){
            if order.is_none() && non_pending_blocks.len() > 0{
                let should_probe = i == 0;
                let new_order = if should_probe {
                    get_order_probe(&mut known_addrs, &mut un_measured_addrs, &mut addr_heap, &mut non_pending_blocks)
                } else {
                    get_order_download(&mut known_addrs, &mut un_measured_addrs, &mut addr_heap, &mut non_pending_blocks)
                };

                order_chans[i].send(Some(new_order.clone())).unwrap();

                *order = Some(new_order);
            }
        }

        //gets responses
        let res = res_rcvr.recv().unwrap();
        match res.response{
            ResponseContent::BlocksFilled(blocks_filled) => {
                for block in &blocks_filled{
                    not_downloaded_blocks.remove(&block);
                }

                let order = thread_orders[res.thread_num].as_ref().unwrap();

                let order_blocks = order.blocks.as_ref().unwrap(); //100% exists
                if blocks_filled.len() == order_blocks.len() {
                    addr_heap.push(Reverse(Addr{address: thread_orders[res.thread_num].as_ref().unwrap().addr.clone(), dur: res.elapsed})); 

                } else {
                    for block in order_blocks{
                        if not_downloaded_blocks.contains(&block){
                            non_pending_blocks.push_back(block.clone());
                        }
                    }
                    //dont add the addr back
                }
            },
            ResponseContent::Torrent(torrent_res) => {
                if let Some(torrent_res) = torrent_res {
                    if !known_addrs.contains(&torrent_res.addr1){
                        un_measured_addrs.push_back(Addr{address: torrent_res.addr1.clone(), dur: Duration::MAX}); 
                        known_addrs.push_front(torrent_res.addr1);
                    }

                   if !known_addrs.contains(&torrent_res.addr2){
                        un_measured_addrs.push_back(Addr{address: torrent_res.addr2.clone(), dur: Duration::MAX}); 
                        known_addrs.push_front(torrent_res.addr2);
                    }
                } 
            },

        } 
        thread_orders[res.thread_num] = None;
    }

    write_file(blocks, &file_name);

    //reap threads 
    for i in 0..NUM_THREADS{
        order_chans[i].send(None).unwrap();
    }
    for thread in threads{
        thread.join().expect("Thread panicked");
    }
}

//TODO make some "state" so we can pass this around better 200000 char function header here
fn get_order_probe(known_addrs: &mut VecDeque<String>, un_measured_addrs: &mut VecDeque<Addr>, addr_heap: &mut BinaryHeap<Reverse<Addr>>, non_pending_blocks: &mut VecDeque<usize>) -> Order {
    if let Some(addr) = un_measured_addrs.pop_front(){
        let mut blocks = Vec::new();
        for _ in 0..BLOCKS_PER_ORDER{
            if let Some(block) = non_pending_blocks.pop_front(){
                blocks.push(block);
            }
        }
        return Order{addr: addr.address, blocks: Some(blocks)};
    }
    if known_addrs.len() < 6 {
        //send torrent msg 
        let ret_addr = known_addrs.pop_front().unwrap(); //unwrap safe bc always has init msg
        known_addrs.push_back(ret_addr.clone()); //just to cycle the list by 1 each time
        return Order{addr: ret_addr, blocks: None}; 
    }

    //just be a regular order (ik it repeats some checks in theory but it will never actually run the repeat code)
    return get_order_download(known_addrs, un_measured_addrs, addr_heap, non_pending_blocks);
}

fn get_order_download(known_addrs: &mut VecDeque<String>, un_measured_addrs: &mut VecDeque<Addr>, addr_heap: &mut BinaryHeap<Reverse<Addr>>, non_pending_blocks: &mut VecDeque<usize>) -> Order {
    if let Some(Reverse(addr)) = addr_heap.pop(){
        let mut blocks = Vec::new();
        for i in 0..BLOCKS_PER_ORDER{
            if let Some(block) = non_pending_blocks.pop_front(){
                blocks.push(block);
            }
        }
        return Order{addr: addr.address, blocks: Some(blocks)};
    }

    if let Some(addr) = un_measured_addrs.pop_front(){
        let mut blocks = Vec::new();
        for i in 0..BLOCKS_PER_ORDER{
            if let Some(block) = non_pending_blocks.pop_front(){
                blocks.push(block);
            }
        }
        return Order{addr: addr.address, blocks: Some(blocks)};
    }

    let ret_addr = known_addrs.pop_front().unwrap(); //unwrap safe bc always has init msg
    known_addrs.push_front(ret_addr.clone()); //just to cycle the list by 1 each time
    return Order{addr: ret_addr, blocks: None}; 

}

fn worker_thread(thread_num: usize, order_channel: mpsc::Receiver<Option<Order>>, res_chan: mpsc::Sender<OrderResponse>, blocks: Arc<Vec<Mutex<Option<Block>>>>, file_name: &str){
    loop {
        let received = order_channel.recv().unwrap();  
        let Some(order) = received else {return;};

        match order.blocks{
            Some(order_blocks) => {
                let start = SystemTime::now();
                let blocks_filled = get_blocks(&order.addr, file_name, blocks.clone(), &order_blocks);
                let Ok(elapsed) = start.elapsed() else {println!("error with timer"); return;};
                res_chan.send(OrderResponse {thread_num, elapsed, response: ResponseContent::BlocksFilled(blocks_filled)}).unwrap(); 
            }

            None => {
                res_chan.send(OrderResponse{thread_num, elapsed: Duration::ZERO, response: ResponseContent::Torrent(torrent_msg(&order.addr, file_name))}).unwrap();
            }
        } 
    }
}

//returns the blocks that were filled 
fn get_blocks(addr: &str, file_name: &str, blocks: Arc<Vec<Mutex<Option<Block>>>>,  block_nums: &Vec<usize>)-> Vec<usize>{
    let stream = TcpStream::connect(addr);
    if stream.is_err(){return vec![];}
    let mut stream = stream.unwrap();  
    let mut ret = Vec::with_capacity(block_nums.len());

    for i in block_nums{
        if let Some(block) = get_block(file_name, i, &mut stream){
            if let Some(mutex_block) = blocks.get(*i){
                ret.push(*i); 
                let mut opt_block = mutex_block.lock().unwrap();
                *opt_block = Some(block);
            }
        }else{
            //addr is prob bad
            break
        }
    }
    ret
}

fn get_block(file_name: &str, block_num: &usize, stream: &mut TcpStream) -> Option<Block>{
    let to_send = format!("GET {0}:{1}\n", file_name, block_num);
    if stream.write(to_send.as_bytes()).is_err(){println!("did not write"); return None;};

    //read in response
    let mut buf : Vec<u8> = vec![];
    let mut temp_buf = [0u8; 11000];

    let max_read_time = Duration::from_millis(5000);
    if stream.set_read_timeout(Some(max_read_time)).is_err(){ return None; }

    let mut seen_new_lines = 0;
    let mut bytes_to_read = -1;
    let mut read_bytes = 0;
    let mut file_size = 0;

    loop{
        match stream.read(&mut temp_buf){
            Ok(i) => {
                read_bytes += i;
                for byte_idx in 0..i{
                    if temp_buf[byte_idx] == ('\n' as u8){
                        seen_new_lines += 1;
                    }
                    buf.push(temp_buf[byte_idx]);
                }
                //either process or break
                if seen_new_lines >= 4{
                    if bytes_to_read == -1{
                        //need to process header 
                        if buf[0] == '4' as u8{
                            return None;
                        }

                        let mut new_lines: Vec<usize> = vec![];

                        //read until we hit the 4th newline
                        for (idx, val) in buf.iter().enumerate(){
                            if val == &('\n' as u8){
                                new_lines.push(idx);
                            }
                            if new_lines.len() == 4 {break;}
                        } 

                        let size_line = &buf[new_lines[1]..new_lines[2]];
                        let size_line = str::from_utf8(size_line);
                        if size_line.is_err(){return None;}
                        let size_line = size_line.unwrap();

                        file_size = get_nums_in_line(size_line);

                        bytes_to_read = file_size as i32 + new_lines[3] as i32;
                    }
                    if bytes_to_read <= read_bytes as i32 && bytes_to_read != -1{
                        break;
                    }
                }

                if i == 0 {break;};

            }
            Err(_) => { break; }
        }

        if buf[0] == '4' as u8{ return None; }
    }

    if bytes_to_read == -1 {return None;} 
    if buf.len() < bytes_to_read as usize {return None;} 
    if buf[0] == '4' as u8{ return None; }
    if (bytes_to_read - file_size as i32 + 1) < 0 || (bytes_to_read+1) as usize > buf.len() {return None;}

    return Some(Block(buf[bytes_to_read as usize-file_size+1 as usize .. bytes_to_read as usize + 1].to_vec()));
}

fn torrent_msg(addr: &str, file_name: &str) -> Option<TorrentResponse>{
    let Ok(socket) = UdpSocket::bind("0.0.0.0:0") else{
        println!("could not bind");
        return None;
    };

    if socket.set_read_timeout(Some(Duration::from_millis(700))).is_err(){
        println!("could not set time out ");
        return None;
    }

    let Ok(_) = socket.send_to(format_torrent_msg!(file_name).as_bytes(), &addr) else{
        print!("failed to send");
        return None;
    };


    let mut buff = [0u8; 2048];

    let Ok((_, _)) = socket.recv_from(&mut buff) else{ print!("could not read"); return None; };

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

    if newlines.len() != 6 || cols.len() != 6{
        return None; //bad format data
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

const INIT_MSG_ATTEMPTS : usize = 100;
//the idea behind this function is that, for the first msg, we really cant have it failing
fn init_torrent_msg(addr: &str, file_name: &str) -> TorrentResponse{
    for i in 0..INIT_MSG_ATTEMPTS{
        if let Some(response) = torrent_msg(addr, file_name){
            return response;
        }
    }

    panic!("could not contact the torrent server");
}


fn write_file(blocks: Arc<Vec<Mutex<Option<Block>>>>, file_name: &str){
    let mut file = File::create(file_name).expect("Failed to create or open the file");
    for block in blocks.iter(){
        let mut block = block.lock().unwrap();
        let Block(to_write) = block.as_mut().expect("missing block when writing");
        file.write_all(&to_write).expect("issue writing block");
    }
}
