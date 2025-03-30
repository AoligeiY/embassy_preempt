set architecture arm
dashboard -layout registers stack assembly source 
target remote :3333
monitor reset halt
load

dashboard -layout source assembly registers stack variables
dashboard source -style height 15
dashboard assembly -style height 15
#dashboard registers -style height 15
#dashboard stack -style height 15