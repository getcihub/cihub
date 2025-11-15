package mock

//go:generate mockgen -package=mock -destination=mock_gen.go github.com/getcihub/cihub/core InstallationStore,InstallationService,JobStore,Refresher,RunnerStore,Scheduler,Session,UserService,UserStore
