project(
	'winit-module',
	'c',
	default_options: [
		'c_std=gnu11',
		'b_lundef=false',
	],
)

xorg = dependency('xorg-server')

library(
	'winit', [
		'src/main.c',
		'src/video.c',
		'src/input.c',
	],
	dependencies: xorg,
	install: true,
	install_dir: meson.global_source_root() / 'install',
	c_args: '-fvisibility=hidden',
)
