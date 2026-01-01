import grpc
import log_service_pb2
import log_service_pb2_grpc
import time
import random
from datetime import datetime

def simple_client(client_id):
    channel = grpc.insecure_channel('127.0.0.1:9021')
    stub = log_service_pb2_grpc.LogServiceStub(channel)
    i= 0 
    while True:
        try:
            i += 1
            # Create log message
            request = log_service_pb2.LogRequest(
                timestamp=str(datetime.utcnow()),
                hostname="py-client-grpc",
                logger_name="logger_grpc", 
                module="Module",
                level=log_service_pb2.Level.INFO,  # Now using enum
                filename="Filename.py",
                function_name="FunctionName",
                line_number="LineNumber",
                message="Message",
                path_name="PathName",
                process_id="ProcessId",
                process_name="ProcessName",
                thread_id="ThreadId",
                thread_name="ThreadName",
                service_name="ServiceName",
                stack_trace="StackTrace"
            )
            
            
            # Send to server
            _ = stub.LogMessage(request)
            
        except Exception as e:
            print(f"Client {client_id}: Error - {e}")
            break


if __name__ == "__main__":
    simple_client(0)