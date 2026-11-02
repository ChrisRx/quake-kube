// Stand-ins from OpenArena (GPLv2) for retail Quake 3 shaders.

textures/base_floor/metfloor1
{
	surfaceparm metalsteps
	{
		map textures/base_floor/metfloor1.tga
		rgbGen const ( 1 1 1 )
	}
	{
		clampmap textures/base_wall/chrome_env2.tga
		blendfunc add
		rgbGen const ( 0.188235 0.188235 0.188235 )
		tcGen environment 
	}
	{
		map textures/base_floor/metfloor1.tga
		blendfunc filter
		rgbGen const ( 0.737255 0.737255 0.737255 )
	}
	{
		map $lightmap 
		blendfunc filter
		tcGen lightmap 
	}
}

textures/base_wall/concrete
{
	{
		map textures/base_wall/concrete.tga
		rgbGen identity
	}
	{
		map $lightmap 
		blendfunc filter
		rgbGen identity
		tcGen lightmap 
	}
	{
		map gfx/fx/detail/d_conc.tga
		blendfunc gl_dst_color gl_src_color
		tcMod scale 8 8
	}
}

textures/gothic_block/killblock_i2
{
	{
		map textures/gothic_block/killblock_i2.tga
		rgbGen identity
	}
	{
		map $lightmap 
		blendfunc filter
		rgbGen identity
		tcGen lightmap 
	}
	{
		map gfx/fx/detail/d_stone.tga
		blendfunc gl_dst_color gl_src_color
		tcMod scale 0.1 0.1
		detail
	}
}

textures/gothic_floor/largerblock3b3dim
{
	{
		map textures/gothic_floor/largerblock3b3dim.tga
		rgbGen identity
	}
	{
		map $lightmap 
		blendfunc filter
		rgbGen identity
		tcGen lightmap 
	}
	{
		map gfx/fx/detail/d_stone.tga
		blendfunc gl_dst_color gl_src_color
		tcMod scale 8 8
		detail
	}
}

textures/gothic_light/skulllight01
{
	qer_editorimage textures/gothic_light/skulllight01.tga
	q3map_lightimage textures/gothic_light/s_light2_blend.tga
	surfaceparm nomarks
	q3map_surfacelight 300
	{
		map textures/gothic_light/skulllight01.tga
		blendfunc gl_one gl_zero
		rgbgen identity
	}
	{
		map $lightmap
		blendfunc filter
		tcgen lightmap
	}
	{
		map textures/gothic_light/s_light2_blend.tga
		blendfunc add
		rgbGen wave sin 0.25 0.25 0 .5
	}
}

textures/gothic_trim/baseboard10_f
{
	{
		map textures/gothic_trim/baseboard10_f.tga
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

textures/gothic_trim/metalsupport4g_1
{
	{
		map textures/gothic_trim/metalsupport4g_1.tga
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

textures/gothic_trim/metalsupport4i
{
	{
		map textures/gothic_trim/metalsupport4i.tga
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

textures/gothic_wall/streetbricks
{
	{
		map textures/gothic_wall/streetbricks.tga
		rgbGen identity
	}
	{
		map $lightmap 
		blendfunc filter
		rgbGen identity
		tcGen lightmap 
	}
	{
		map gfx/fx/detail/d_stone.tga
		blendfunc gl_dst_color gl_src_color
		tcMod scale 8 8
		detail
	}
	{
		map gfx/fx/detail/d_rock.tga
		blendfunc gl_dst_color gl_src_color
		tcMod scale 4 4
		detail
	}
}

textures/organics/grass3
{
	{
		map textures/organics/grass3.tga
		rgbGen identity
	}
	{
		map $lightmap 
		blendfunc filter
		rgbGen identity
		tcGen lightmap 
	}
	{
		map gfx/fx/detail/d_sandy.tga
		blendfunc gl_dst_color gl_src_color
		tcMod scale 8 8
		detail
	}
}

