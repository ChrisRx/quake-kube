package must

// Must
func Must[T any](a T, err error) T {
	if err != nil {
		panic(err)
	}
	return a
}
