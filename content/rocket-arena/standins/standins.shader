// Stand-ins from OpenArena (GPLv2) for retail Quake 3 shaders.

textures/gothic_trim/metalsupport4i_bit
{
	{
		map textures/gothic_trim/metalsupport4i_bit.tga
		rgbGen identity
	}
	{
		map $lightmap 
		blendfunc filter
		rgbGen identity
		tcGen lightmap 
	}
	{
		map gfx/fx/detail/d_met.tga
		blendfunc gl_dst_color gl_src_color
		tcMod scale 8 8
		detail
	}
}

textures/gothic_trim/pitted_rust3_black
{
	{
		map textures/gothic_trim/pitted_rust3_black.tga
		rgbGen identity
	}
	{
		map $lightmap 
		blendfunc filter
		rgbGen identity
		tcGen lightmap 
	}
	{
		map gfx/fx/detail/d_met.tga
		blendfunc gl_dst_color gl_src_color
		tcMod scale 8 8
		detail
	}
}

