import json
import pprint

from urllib.request import urlopen


if __name__ == "__main__":
    response = urlopen("http://localhost:8000/user/auth", b"l6z985dz5k4jhgn265c6291r")
    pprint.pprint(json.loads(response.read().decode("utf8")))
