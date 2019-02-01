import json
import pprint

from urllib.request import Request, urlopen


if __name__ == "__main__":
    response = urlopen("http://localhost:8000/user/auth", b"1hlevn5o7uhuj398h6t2nu79")
    pprint.pprint(json.loads(response.read().decode("utf8")))
