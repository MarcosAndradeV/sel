(print "--- Testing OS Environment ---")
(define user (os/getenv "USER"))
(print (if (nil? user) "USER not set" user))

(print "--- Testing OS Args ---")
(print (os/args))

(print "--- Testing File I/O ---")
(define filename "tests/test_output.txt")
(io/write-string filename "Hello from sel Native I/O!")
(print (io/file-exists? filename))
(print (io/read-string filename))
