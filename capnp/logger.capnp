#// ! \\// ! \\// ! \\// ! \\// ! \\// ! \\// ! \\// ! \\// ! \\// ! \\// ! \\// ! \\// ! \\// ! \\
#       // ! \\         No underscore in label field name !!!                        // ! \\
#       // ! \\         field name camelCase                                         // ! \\
#       // ! \\         field number start = 0                                       // ! \\
#       // ! \\         enum number start = position                                       // ! \\
#// ! \\// ! \\// ! \\// ! \\// ! \\// ! \\// ! \\// ! \\// ! \\// ! \\// ! \\// ! \\// ! \\// ! \\


# generate notifMsg ncap proto
# ncap install is required :
# go get -u -t zombiezen.com/go/capnproto2
# GO111MODULE=off go get -u capnproto.org/go/capnp/v3/

# cd "/users/IMac/Desktop/govenv/api/capnp/LoggerMsg"
# capnp compile -I "/users/IMac/Desktop/govenv/api/capnp/loggerMsg/go-capnp/std" -ogo logger.capnp


#using Go = import "/go.capnp";
@0xfc867d59c4f2e15c;
#$Go.package("MyLogger");
#$Go.import("govenv/pkg/common/MyLogger");

enum Level {
  notset @0;
  debug @1;
  stream @2;
  info @3;
  logon @4;
  logout @5;
  trade @6;
  schedule @7;
  report @8;
  warning @9;
  error @10;
  critical @11;
}

struct LoggerMsg {
  # // recorded by log_server : 
  timestamp @0 :Text;
  hostname @1 :Text;
  loggerName @2 :Text;
  module @3 :Text;
  level @4 :Level;
  filename @5 :Text;
  functionName @6 :Text;
  lineNumber @7 :Text;
  message @8 :Text;

  # // others

  # // path of the file  
  pathName @9 :Text;     

  # // Process information
  processId @10 :Text;    
  processName @11 :Text;
  
  # // Thread information
  threadId @12 :Text;
  threadName @13 :Text;
  
  # // Additional requested fields
  serviceName @14 :Text; 

  # // Optional stack trace for errors
  stackTrace @15 :Text;   
}