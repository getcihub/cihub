package mock

//go:generate mockgen -package=mock -destination=mock_gen.go github.com/getcihub/cihub/core Session,UserService,UserStore
