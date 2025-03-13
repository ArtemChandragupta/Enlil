import socket
import random

def start_server(ip, port):
    server_socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    server_socket.bind((ip, port))
    server_socket.listen(1)
    print(f"Server listening on {ip}:{port}")

    while True:
        client_socket, addr = server_socket.accept()
        print(f"Connection from {addr}")
        data = client_socket.recv(1024).decode()
        print(f"Received data: {data}")

        # Эмуляция данных
        if ip == '127.0.0.27':
            response = f"{random.uniform(20.0, 30.0):.2f}"  # Температура сопла
        elif ip == '127.0.0.28':
            response = f"{random.uniform(20.0, 30.0):.2f}"  # Температура конуса
        elif ip == '127.0.0.203':
            response = ' '.join([f"{random.uniform(10.0, 20.0):.2f}" for _ in range(16)])  # Давления от сканера 203
        elif ip == '127.0.0.204':
            response = ' '.join([f"{random.uniform(10.0, 20.0):.2f}" for _ in range(16)])  # Давления от сканера 204

        client_socket.send(response.encode())
        client_socket.close()

# Запуск серверов
import threading

threads = []
for ip in ['127.0.0.27', '127.0.0.28', '127.0.0.203', '127.0.0.204']:
    t = threading.Thread(target=start_server, args=(ip, 9000))
    t.start()
    threads.append(t)

for t in threads:
    t.join()

