struct Point {
    float x;
    float y;
};

struct Rect {
    struct Point pos;
    float w;
    float h;
};

float get_distance_sq(struct Point p) {
    return p.x * p.x + p.y * p.y;
}

struct Point make_point(float x, float y) {
    struct Point p;
    p.x = x;
    p.y = y;
    return p;
}

float get_rect_area(struct Rect r) {
    return r.w * r.h;
}

struct Rect make_rect(float x, float y, float w, float h) {
    struct Rect r;
    r.pos.x = x;
    r.pos.y = y;
    r.w = w;
    r.h = h;
    return r;
}
