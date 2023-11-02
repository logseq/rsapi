const fs = require("fs");
const rsapi = require("../../rsapi");

const idToken =
  "eyJraWQiOiJZeFltRzdXMDBNM3RkOHVNSUlReGNGdUxHRTl5YXphbzdwaEZMdHNKcjAwPSIsImFsZyI6IlJTMjU2In0.eyJhdF9oYXNoIjoiR0p2N19GcE9jMWNPU1ZwWm5qYjd1USIsInN1YiI6IjZlOGU1ZWJhLTRlNWEtNDFiZS1iYmYwLTRiOWM4MzNkNWFlZCIsImVtYWlsX3ZlcmlmaWVkIjp0cnVlLCJpc3MiOiJodHRwczpcL1wvY29nbml0by1pZHAudXMtZWFzdC0yLmFtYXpvbmF3cy5jb21cL3VzLWVhc3QtMl9ZTjc4QU1HSDMiLCJjb2duaXRvOnVzZXJuYW1lIjoiNmU4ZTVlYmEtNGU1YS00MWJlLWJiZjAtNGI5YzgzM2Q1YWVkIiwiYXVkIjoiNGZpNzllbjlhdXJjbGtiOTJlMjVobXU5dHMiLCJldmVudF9pZCI6IjhkYTIwYjE2LTVmNDAtNDU3Mi04YTgyLTk5ZTQ5ZDZkYzYxNiIsInRva2VuX3VzZSI6ImlkIiwiYXV0aF90aW1lIjoxNjQwODYxMDM4LCJleHAiOjE2NDA5NDc0MzgsImlhdCI6MTY0MDg2MTAzOCwianRpIjoiNDZmMDAyNDYtZTg0MS00ODVjLThlNGYtZjI5MTlmNDRmZTNmIiwiZW1haWwiOiJhbmRlbGZAZ21haWwuY29tIn0.tTjz0aZaXh5cB7VLfw99paVewRHfhILR9YHvO4wmEzGaAn7bM_pVOw0tnFq-B1gptujmye1GoY4jPZ5zFHdA4mmaCBTackBQ_oIWhirrs-eZIYbwkgolpPaD8gLMIqZYgEKzJP9f3waMrBwPYDuyKodM_EqKA6OtPnt1Qs_vteo6U83uwalFpiiwoe1mgQTDWdrxtNhFtuvn1BrxxlP5bM9ZB4AJGC3__L_q1NCGv1Q5f2Uuv9GkHVek9fOil3WFqXoJJF0Aj4rbrRxsmJM3rIvWeRKJLB-QFtoVq5wg7ru_UuCFnO62z8RUBYDqKceESarI-UelT2SvgOE4-yleNQ";
const graphUUID = "23de3a01-4ece-442b-9a70-bb23ceeac9a5";


console.log(rsapi);

async function main() {
  const basePath = await rsapi.canonicalizePath(__dirname + "/../");
  console.log("basePath:", basePath);

  const meta = await rsapi.getLocalFilesMeta(graphUUID, basePath, [
    "build.rs",
    "Cargo.toml",
  ]);
  console.log(meta);

  const metaAll = await rsapi.getLocalAllFilesMeta(graphUUID, basePath);
  console.log(metaAll);

  //const ret = await rsapi.updateLocalFiles(graphUUID, basePath, ["pages/first.md"], idToken);
  //console.log("update local", ret);


  //const remoteUpdate = await rsapi.updateRemoteFile(graphUUID, basePath, "package.json", 15, idToken);
  const xx = await rsapi.updateRemoteFile("bc5e1ced-96b1-4418-abea-4536cce9a35f", basePath, "xx.md", 34, "eyJraWQiOiJqRUJVYUl6Y1VTNExkbDdhdlMyNk9wbTQyZm5HclB0dWlCU3Nuam1NVlRjPSIsImFsZyI6IlJTMjU2In0.eyJzdWIiOiI4MDBlZGQ1ZS1mYzJiLTQzNTItYTIyMS1jYTE2ZTZmYjQxNzciLCJpc3MiOiJodHRwczpcL1wvY29nbml0by1pZHAudXMtZWFzdC0yLmFtYXpvbmF3cy5jb21cL3VzLWVhc3QtMl9ZTjc4QU1HSDMiLCJ2ZXJzaW9uIjoyLCJjbGllbnRfaWQiOiI0Zmk3OWVuOWF1cmNsa2I5MmUyNWhtdTl0cyIsIm9yaWdpbl9qdGkiOiI5YzlhYWNmNi0zMDgyLTQwYTctODE1NC02ZjA5Yjg0OTE3YzAiLCJldmVudF9pZCI6IjJjNDAxMjM5LWJmZGYtNDgxZi05MzJkLTQyZjg2OWE4MzdjOSIsInRva2VuX3VzZSI6ImFjY2VzcyIsInNjb3BlIjoicGhvbmUgb3BlbmlkIGVtYWlsIiwiYXV0aF90aW1lIjoxNjQwOTUxMzA4LCJleHAiOjE2NDEwMzc3MDgsImlhdCI6MTY0MDk1MTMwOCwianRpIjoiOGNkMTY5MzYtOTZiYy00NzMyLTkxOWQtODU4MTY3ZDI4MjU5IiwidXNlcm5hbWUiOiI4MDBlZGQ1ZS1mYzJiLTQzNTItYTIyMS1jYTE2ZTZmYjQxNzcifQ.boDmtFb04tu3YtKDW3eGaaRW9jCiAEzxWlpE3mBDV2xv4_3OBm_-NGc-NFzIr8wP1ZrMDY4AZVN7OHMfzHJ6czzJhksBPdbHKfp8EpOd_pyHBmKH0kAV-RNjxAa5GgOe1nkqJkRqW_YXxXdV463yUV1ORLcgL-7GyGgsJGe8xpbR_DiMpm1al7aEHxgDinW2OXNT9YCM6EuuHM9pBj33rks7m_QCj92HACcpWw6Rv3TNbx183FRyKSFGQW_1dbdJUAj_sn2gIBF7hvFyab0_Ibvzh0iU2Avrc40B6wrDihbsi8lIsi5vlEv7M3k70Ewhhl8Iz0xEy5Pb-ByCHSVXLA");
  console.log(xx);

}

main()
  .then(() => {
    console.log("ok");
  })
  .catch((err) => {
    console.log("error:", err);
  });
