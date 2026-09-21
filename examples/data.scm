; break :: (a -> Bool) -> [a] -> [a] . [a]
(defn (break p xs)
    ((_ xs) :when (empty? xs) :do (list nil nil))
    ((p (cons x xs-p)) :when (p x) :do (list nil xs))
    ((_ (cons x xs-p)) :do (match (break p xs-p) ((ys zs) :do (list (cons x ys) zs)))))

; lines :: String -> [String]
(defn lines
    ("" "")
    (s :do (match (break \(c) (eq? c #\newline) s)
            ((l s-p) :do (cons l (match s-p
                (() :do "")
                ((cons _ s-pp) :do (lines s-pp))))))))

; unlines :: [String] -> String
(defn unlines
    (() nil)
    ((cons l ls) (append (cons #\newline l) (unlines ls))))
