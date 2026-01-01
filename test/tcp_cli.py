import capnp
import socket
import datetime

from os import getcwd as osGetcwd
from os.path import join as osPathJoin
from socket import gethostname
data_msg = capnp.load(osPathJoin(osGetcwd(), 'logger.capnp'))

# Load the Cap'n Proto schema

import struct
class SafeSocket():
    Name = "safeSocket"
    def __init__(self, conn, name=None, serializer_class=None):
        if not name is None: self.Name = name
        self.struct = struct
        self.conn = conn
    def send_data(self, serialized_data):
        self.conn.sendall(self.struct.pack('>L', len(serialized_data)))
        self.conn.sendall(serialized_data)
    def receive_data(self):         
        chunk = self.conn.recv(4)
        if len(chunk) < 4:
            return False
        slen = self.struct.unpack('>L', chunk)[0]
        chunk = self.conn.recv(slen)
        while len(chunk) < slen:
            chunk = chunk + self.conn.recv(slen - len(chunk))
        return self.deSerialize(chunk)
    def __enter__(self):
        return self
    def __exit__(self, *args):
        if not self.conn._closed:
            self.conn.close()

def send_capnp_message():
    # Create a Cap'n Proto message
    from datetime import datetime
    logger_msg = data_msg.LoggerMsg.new_message()
    logger_msg.timestamp          = str(datetime.utcnow()) #"2025-01-28T12:34:56Z" #string // When the event occurred
    logger_msg.hostname           = "py-client" #string // Host/machine name
    logger_msg.loggerName         = "LoggerTCP" #string // Name of the logger (usually __name__)
    logger_msg.module             = "Module" #string // Module (name portion of filename)
    # the proto capnp used the proto format for enum (0, 1, 2, 3, 4, 5)
    logger_msg.level              = data_msg.Level.warning #= log_level_to_int[Level]/10
    logger_msg.filename           = "Filename.py" #string // Filename portion of pathname
    logger_msg.functionName       = "FunctionName" #string // Function name
    logger_msg.lineNumber         = "LineNumber" #string // Source line number
    logger_msg.message            = "Message" #string // The log message
    #logger_msg.relativeCreated    = "RelativeCreated" #string // difference between logging triggered and loggerMessage creation
    logger_msg.pathName           = "PathName" #string // Full pathname of the source file
    logger_msg.processId          = "ProcessId" #string // Process ID
    logger_msg.processName        = "ProcessName" #string // Process name
    logger_msg.threadId           = "ThreadId" #string // Thread ID               
    logger_msg.threadName         = "ThreadName" #string // Thread name
    logger_msg.serviceName        = "ServiceName" #string // Name of the service generating the log
    logger_msg.stackTrace         = "StackTrace" #string // Stack trace if available

    # Serialize the message
    return logger_msg.to_bytes_packed()


if __name__ == "__main__":
    from time import sleep
    try:
        with socket.create_connection(("127.0.0.1", 9020)) as sock:
            logSock = SafeSocket(sock)
            message_count = 0
            
            while True:
                serialized_message = send_capnp_message()
                logSock.send_data(serialized_message)
                message_count += 1
                
    except Exception as e:
        print(f"Client {1}: Error - {e}")