struct CameraUniform {
	view_proj: mat4x4<f32>,
	view_pos: vec3<f32>,
}

struct Light {
	position: vec3<f32>,
	color: vec3<f32>,
	intensity: f32,
}

struct VertexInput {
	@location(0) position: vec3<f32>,
	@location(1) normal: vec3<f32>,
	@location(2) tex_coords: vec2<f32>,
	@location(3) tangent: vec3<f32>,
}

struct InstanceInput {
	@location(4) model_matrix_0: vec4<f32>,
	@location(5) model_matrix_1: vec4<f32>,
	@location(6) model_matrix_2: vec4<f32>,
	@location(7) model_matrix_3: vec4<f32>,
}

struct VertexOutput {
	@builtin(position) clip_position: vec4<f32>,
	@location(0) tex_coords: vec2<f32>,
	@location(1) world_position: vec3<f32>,
	@location(2) tbn_matrix_0: vec3<f32>,
	@location(3) tbn_matrix_1: vec3<f32>,
	@location(4) tbn_matrix_2: vec3<f32>,
	@location(5) normal: vec3<f32>,
}

@group(1) @binding(0)
var<uniform> camera: CameraUniform;

@group(2) @binding(0)
var<uniform> lights: array<Light, 2>;

fn calculate_tangent(normal: vec3<f32>) -> vec3<f32> {
	var helper: vec3<f32>;

	if (abs(normal.y) > 0.99) {
		helper = vec3<f32>(1.0, 0.0, 0.0);
	} else {
		helper = vec3<f32>(0.0, 1.0, 0.0);
	}

	return normalize(cross(normal, helper));
}

@vertex
fn vs_main(model: VertexInput, instance: InstanceInput) -> VertexOutput {
	let model_matrix = mat4x4<f32>(
		instance.model_matrix_0,
		instance.model_matrix_1,
		instance.model_matrix_2,
		instance.model_matrix_3,
	);

	var out: VertexOutput;

	out.tex_coords = model.tex_coords;
	out.clip_position = camera.view_proj * model_matrix * vec4<f32>(model.position, 1.0);
	out.world_position = (model_matrix * vec4<f32>(model.position, 1.0)).xyz;
	
	let world_tangent = normalize((model_matrix * vec4<f32>(model.tangent, 0.0)).xyz);
	let world_normal = normalize((model_matrix * vec4<f32>(model.normal, 0.0)).xyz);
	let world_bitangent = normalize(cross(world_normal, world_tangent));
	let tbn = mat3x3<f32>(world_tangent, world_bitangent, world_normal);

	out.tbn_matrix_0 = tbn[0];
	out.tbn_matrix_1 = tbn[1];
	out.tbn_matrix_2 = tbn[2];

	out.normal = world_normal;

	return out;
}

@group(0) @binding(0) var t_diffuse: texture_2d<f32>;
@group(0) @binding(1) var s_diffuse: sampler;

@group(0) @binding(2) var t_normal: texture_2d<f32>;
@group(0) @binding(3) var s_normal: sampler;

@group(0) @binding(4) var t_metallic_roughness: texture_2d<f32>;
@group(0) @binding(5) var s_metallic_roughness: sampler;

@group(0) @binding(6) var t_ao: texture_2d<f32>;
@group(0) @binding(7) var s_ao: sampler;

@group(3) @binding(0) var<uniform> t_irradiance: texture_cube<f32>;
@group(3) @binding(1) var<uniform> s_irradiance: sampler;

const pi = radians(180.0);

fn geometry_schlick_ggx(cos_theta: f32, roughness: f32) -> f32 {
	let r = roughness + 1.0;
	let k = (r * r) / 8.0;

	let numerator = cos_theta;
	let denominator = cos_theta * (1.0 - k) + k;

	return numerator / denominator;
}

fn geometry_smith(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, roughness: f32) -> f32 {
	let n_dot_v = max(dot(n, v), 0.0);
	let n_dot_l = max(dot(n, l), 0.0);
	let ggx1 = geometry_schlick_ggx(n_dot_v, roughness);
	let ggx2 = geometry_schlick_ggx(n_dot_l, roughness);

	return ggx1 * ggx2;
}

fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
	return f0 + (vec3<f32>(1.0) - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

fn fresnel_schlick_roughness(cos_theta: f32, f0: vec3<f32>, roughness: f32) -> f32 {
	return f0 + (max(vec3<f32>(1.0 - roughness), f0) - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

fn distribution_ggx(xi: vec2<f32>, n: vec3<f32>, roughness: f32) -> f32 {
	let a = roughness * roughness;

	let phi = 2.0 * pi * xi.x;
	let cos_theta = sqrt((1.0 - xi.y) / (1.0 + (a * a - 1.0) * xi.y));
	let sin_theta = sqrt(1.0 - cos_theta * cos_theta);

	let h = vec3<f32>(
		sin_theta * cos(phi),
		sin_theta * sin(phi),
		cos_theta,
	);

	// https://learnopengl.com/PBR/IBL/Specular-IBL
}

fn radical_inverse_vdc(bits: u32) -> f32 {
	var bits = (bits << 16) | (bits >> 16);

	bits = ((bits & 0x55555555) << 1) | ((bits & 0xAAAAAAAA) >> 1);
	bits = ((bits & 0x33333333) << 2) | ((bits & 0xCCCCCCCC) >> 2);
	bits = ((bits & 0x0F0F0F0F) << 4) | ((bits & 0xF0F0F0F0) >> 4);
	bits = ((bits & 0x00FF00FF) << 8) | ((bits & 0xFF00FF00) >> 8);

	return f32(bits) * 2.3283064365386963e-10;
}

fn hammersley(i: u32, n: u32) -> vec2<f32> {
	return vec2<f32>(f32(i) / f32(n), radical_inverse_vdc(i));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
	let albedo_raw = textureSample(t_diffuse, s_diffuse, in.tex_coords);
	let albedo = pow(albedo_raw.rgb, vec3<f32>(2.2));
	let normal_map = textureSample(t_normal, s_normal, in.tex_coords).rgb;
	let ao = textureSample(t_ao, s_ao, in.tex_coords).r;

	let metallic_roughness = textureSample(t_metallic_roughness, s_metallic_roughness, in.tex_coords);
	let roughness = metallic_roughness.g;
	let metallic = metallic_roughness.b;

	let tbn = mat3x3<f32>(
		in.tbn_matrix_0,
		in.tbn_matrix_1,
		in.tbn_matrix_2,
	);
	let n = normalize(tbn * (2.0 * normal_map - 1.0));
	let v = normalize(camera.view_pos - in.world_position);
	let f0 = mix(vec3<f32>(0.04), albedo.rgb, metallic);

	var lo = vec3<f32>(0.0);

	for (var i = 0; i < 2; i++) {
		let light = lights[i];

		let l = normalize(light.position - in.world_position);
		let h = normalize(v + l);
		let distance = length(light.position - in.world_position);
		let attenuation = 1.0 / (distance * distance);
		let radiance = light.color * attenuation;

		// cook-torrance brdf
		let ndf = distribution_ggx(n, h, roughness);
		let g = geometry_smith(n, v, l, roughness);
		let f = fresnel_schlick(max(dot(h, v), 0.0), f0);

		let numerator = ndf * g * f;
		let denominator = 4.0 * max(dot(n, v), 0.0) * max(dot(n, l), 0.0) + 0.0001;
		let specular = numerator / denominator;

		// calculate ratio of refraction
		let k_s = f;
		let k_d = (vec3<f32>(1.0) - k_s) * (1.0 - metallic);

		let n_dot_l = max(dot(n, l), 0.0);
		let diffuse = k_d * albedo / pi;

		// add to outgoing radiance Lo
		lo += (diffuse + specular) * radiance * n_dot_l;
	}

	let k_s = fresnel_schlick_roughness(max(dot(n, v), 0.0), f0, roughness);
	let k_d = vec3<f32>(1.0) - k_s;
	let irradiance = textureSample(t_irradiance, s_irradiance, n);
	let diffuse = irradiance * albedo;
	let ambient = (k_d * diffuse) * ao;

	var color = ambient + lo;

	// tone mapping
	color = color / (color + vec3<f32>(1.0));
	// gamma correction
	color = pow(color, vec3<f32>(1.0 / 2.2));

	return vec4<f32>(color, albedo_raw.a);
}

