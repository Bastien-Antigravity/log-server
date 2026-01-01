import grpc
import log_service_pb2
import log_service_pb2_grpc
import multiprocessing
import time
import random
from datetime import datetime

def grpc_worker(client_id, num_messages=20):
    print(f"Client {client_id}: Starting...")
    
    try:
        channel = grpc.insecure_channel('localhost:9021')
        stub = log_service_pb2_grpc.LogServiceStub(channel)
        
        for i in range(num_messages):
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
            print(f"Client {client_id}: Message {i} - OK")
            
            # Random delay between messages
            time.sleep(random.uniform(0.1, 0.5))
            
    except Exception as e:
        print(f"Client {client_id}: Failed - {e}")

def main():
    NUM_CLIENTS = 10
    processes = []
    
    print(f"Starting {NUM_CLIENTS} gRPC clients...")
    
    # Start all clients
    for i in range(NUM_CLIENTS):
        p = multiprocessing.Process(target=grpc_worker, args=(i,))
        processes.append(p)
        p.start()
        time.sleep(0.2)  # Stagger starts
    
    print("All clients started. Waiting for completion...")
    
    # Wait for all to finish
    for p in processes:
        p.join()
    
    print("All clients finished!")

if __name__ == "__main__":
    main()