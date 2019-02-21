import json
import pprint
import socket

from urllib.request import urlopen


if __name__ == "__main__":
    response = urlopen("http://localhost:8000/user/auth", b"ft4pq85ns15b6q57iue11zsd")
    response = json.loads(response.read().decode("utf8"))

    pprint.pprint(response)

    conn = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    conn.connect(("127.0.0.1", 28008))
    conn.send(b"123123123")
