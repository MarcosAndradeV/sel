# List with nil are not working

- STATUS: OPEN
- PRIORITY: 200
- TAGS: bug

(list nil nil) should eval to (nil nil) but currently is not

Bug was found here. Current commit 769cac417689dfbeb9620c1f2f2f6afd95b73c50
```
; break :: (a -> Bool) -> [a] -> ([a],[a])
; HBC version (stolen)
(defn (break p xs)
    ((_ xs) :when (empty? xs) :do (list nil nil))
    ((p (cons x xs-p)) :when (p x) :do (list '() xs))
    ((_ (cons x xs-p)) :do (match (break p xs-p) ((ys zs) :do (list (cons x ys) zs))))
)

(defn lines
    ("" '())
    (s :do (match (break \(c) (eq? c #\newline) s)
            ((l s-p) :do (list l (match s-p
                    (() :do '())
                    ((cons _ s-pp) :do (lines s-pp)))
            ))
        ))
)
```
