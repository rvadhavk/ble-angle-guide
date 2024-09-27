from http.server import HTTPServer, SimpleHTTPRequestHandler
import ssl


httpd = HTTPServer(('127.0.0.1', 5000), SimpleHTTPRequestHandler)
context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
context.load_cert_chain(certfile='cert.pem', keyfile='key.pem', password='')
context.set_ciphers("@SECLEVEL=1:ALL")
httpd.socket = context.wrap_socket(httpd.socket, server_side=True)
try:
    print("Server running. Press Ctrl+C to stop.")
    httpd.serve_forever()
except KeyboardInterrupt:
    print("\nServer stopped.")
finally:
    httpd.server_close()
