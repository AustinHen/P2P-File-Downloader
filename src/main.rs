fn main() {
}

struct TorrentResponse{
    num_blocks: usize,
    file_size: usize,
    addr1: String,
    addr2: String
}

fn torrent_msg(addr: String) -> Option<TorrentResponse>{
   todo!() 
}

struct File{
    blocks: Map, 
}

impl File{
    fn new(){
        todo!()
    }
    fn get_blocks(&mut self, addr: String){
        todo!()
    }

    fn write_file(&mut self, file_name: String){
        todo!()
    }
}

//grabs all blocks from the addr
