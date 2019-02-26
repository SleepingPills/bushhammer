import json
import base64
import pprint
import socket

from io import BytesIO
from urllib.request import urlopen


if __name__ == "__main__":
    response = urlopen("http://localhost:8000/user/auth", b"sq99odqaeuhx9tfqlvnfcpsk")
    response = json.loads(response.read().decode("utf8"))

    pprint.pprint(response)

    data = response["data"]

    token_bytes = BytesIO()

    print("Writing Version: {} bytes", token_bytes.write(base64.b64decode(data["version"])))
    print("Writing Protocol: {} bytes", token_bytes.write(data["protocol"].to_bytes(2, "big")))
    print("Writing Expiry: {} bytes", token_bytes.write(data["expires"].to_bytes(8, "big")))
    print("Writing Sequence: {} bytes", token_bytes.write(data["sequence"].to_bytes(8, "big")))
    print("Writing Private Data: {} bytes", token_bytes.write(base64.b64decode(data["data"])))
    print("Total written: {} bytes", len(token_bytes.getvalue()))

    print("Creating socket")
    conn = socket.socket(socket.AF_INET, socket.SOCK_STREAM)

    print("Connecting to game server")
    conn.connect(("127.0.0.1", 28008))

    print("Sending connectiontoken")
    conn.send(token_bytes.getvalue())
