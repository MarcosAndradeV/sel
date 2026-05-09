(println "--- Testing OS Environment ---")
(define user (os/getenv "USER"))
(println (if (nil? user) "USER not set" user))

(println "--- Testing OS Args ---")
(println (os/args))

(println "--- Testing File I/O ---")
(define filename "tests/test_output.txt")
(io/write-string filename "Hello from sel Native I/O!")
(println (io/file-exists? filename))
(println (io/read-string filename))
