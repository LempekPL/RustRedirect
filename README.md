# Own url shortener or redirector

## Building from source

1. Download project and unpack it or clone it using `git clone https://github.com/LempekPL/RustRedirect`
   with [git](https://git-scm.com/)
2. Open cmd/terminal
3. Go inside project's folder using `cd <path to the folder>`
4. Run `cargo build --release`
5. The executable file should be located in <project folder>/target/release/

## Api endpoints

- GET `/api/v1/redirect` - get list of redirects\
  Hearders:
  Authorization - string key\
  Response: JSON\
  Object{ success: bool, response: Array\[Domain] | string }\
  Domain = {id: number, name: string, domain: string}\
  \
  Example of successful response
  ```json
  {  
    "success": true,
    "response": [
      { "id": 1, "name":"example", "domain":"https://example.com" },
      { "id": 2, "name":"lk", "domain":"https://lmpk.tk" }
    ]
  }
  ```
- POST `/api/v1/redirect/create?name=<name>&domain=<domain>` - create redirect\
  Params:
  name - string, domain - string\
  Hearders:
  Authorization - string key\
  Response: JSON\
  Object{ success: bool, response: string }\
  \
  Example of successful response
  ```json
  {  
    "success": true,
    "response": "Created redirect to <domain> named <name>"
  }
  ```
  Example of unsuccessful response
  ```json
  {  
    "success": false,
    "response": "Server error. Can't connect to the database. Contact the developer"
  }
  ```