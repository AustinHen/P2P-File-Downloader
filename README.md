# P2P
This is a peer to peer file downloaded based on bit torrent. 

Normally files are distributed by a central sever. In a P2P approach, files are broken up into blocks and stored on multiple other user's devices. 

P2P downloading is far more involved than client server downloading: peers can go offline, be missing blocks, or have slow internet. While all of this can happen with a sever, you can't really do anything about it- you just have to wait for the server to come back online. Often peers are far slower and less reliable than servers - they are pcs used by actually people: their main purposes is not to dish out files). 

To download files in a reasonable amount of time you have to be constantly assessing peers to ensure that you are always downloading from the best ones. 

My approach was to have 4 worker threads: 3 of which would attempt to download the block from the fastest available peer. The 4th thread would probe other peers: its goal is to find new, potentially faster, peer. After each chunk of blocks is downloaded the peer's score is updated. 

# Running
this was part of a school project - I though my solution was cool (and very fast) so I am sharing it. The code to distribute the blocks is not mine so I can't share that part. 

To run call 
```console
cargo run [filename] [ip] [port]
```

