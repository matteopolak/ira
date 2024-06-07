pub struct EquiToCubemap {
	pub bind_group_layout: wgpu::BindGroupLayout,
	pub pipeline: wgpu::RenderPipeline,
}

impl EquiToCubemap {
	pub fn new(device: &wgpu::Device) -> Self {
		let shader = device.create_shader_module(wgpu::include_wgsl!("../shaders/cubemap.wgsl"));

		todo!()
	}
}
