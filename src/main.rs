use std::{ffi::CString, fs::DirEntry, mem, time::{self, SystemTime}};

use beryllium::{events, init, video, Sdl};
use dicom_dictionary_std::tags;
use dicom_object::open_file;
use dicom_pixeldata::{ConvertOptions, PixelDecoder, VoiLutOption};
use gl33::{
    global_loader::{
        glActiveTexture, glAttachShader, glBindBuffer, glBindTexture, glBindVertexArray, glBlendFunc, glBufferData, glClear, glClearColor, glCompileShader, glCreateProgram, glCreateShader, glCullFace, glDeleteBuffers, glDeleteProgram, glDeleteShader, glDeleteVertexArrays, glDisableVertexAttribArray, glDrawElements, glEnable, glEnableVertexAttribArray, glFrontFace, glGenBuffers, glGenTextures, glGenVertexArrays, glGenerateMipmap, glGetProgramInfoLog, glGetProgramiv, glGetShaderInfoLog, glGetShaderiv, glGetUniformLocation, glLinkProgram, glShaderSource, glTexImage3D, glTexParameterfv, glTexParameteri, glUniform1i, glUniformMatrix4fv, glUseProgram, glVertexAttribPointer, load_global_gl
    }, GL_ARRAY_BUFFER, GL_BACK, GL_BLEND, GL_CCW, GL_CLAMP_TO_BORDER, GL_CLAMP_TO_EDGE, GL_COLOR_BUFFER_BIT, GL_COMPILE_STATUS, GL_CULL_FACE, GL_ELEMENT_ARRAY_BUFFER, GL_FLOAT, GL_FRAGMENT_SHADER, GL_FRONT, GL_GREEN, GL_INFO_LOG_LENGTH, GL_LINEAR, GL_LINEAR_MIPMAP_LINEAR, GL_LINK_STATUS, GL_NEAREST, GL_NEAREST_MIPMAP_NEAREST, GL_ONE_MINUS_SRC_ALPHA, GL_R8, GL_READ_BUFFER, GL_RED, GL_SRC_ALPHA, GL_STATIC_DRAW, GL_TEXTURE0, GL_TEXTURE_3D, GL_TEXTURE_BORDER_COLOR, GL_TEXTURE_MAG_FILTER, GL_TEXTURE_MIN_FILTER, GL_TEXTURE_WRAP_R, GL_TEXTURE_WRAP_S, GL_TEXTURE_WRAP_T, GL_TRIANGLES, GL_UNSIGNED_BYTE, GL_UNSIGNED_INT, GL_VERTEX_SHADER
};
use image::EncodableLayout;

const DIR_NAME: &'static str = r"D:\dicom_data\DICOM\";

#[rustfmt::skip]
fn get_cube_vertices() -> [f32; 48] {
    [
        // front face pos       tex
        -0.3, -0.3, -0.3,       0.0, 0.0, 0.0, // 0
         0.3, -0.3, -0.3,       1.0, 0.0, 0.0, // 1
         0.3,  0.3, -0.3,       1.0, 1.0, 0.0, // 2
        -0.3,  0.3, -0.3,       0.0, 1.0, 0.0, // 3

        // right face 
        0.3, -0.3, 0.3,         1.0, 0.0, 1.0, // 4
        0.3,  0.3, 0.3,         1.0, 1.0, 1.0, // 5

        // left face
        -0.3, -0.3, 0.3,         0.0, 0.0, 1.0, // 6
        -0.3,  0.3, 0.3,         0.0, 1.0, 1.0, // 7
    ]
}

#[rustfmt::skip]
fn get_cube_indices() -> [u32; 36] {
    [
        // front face
        0, 1, 2,
        0, 2, 3,
        // right face
        1, 4, 5,
        1, 5, 2,
        // left face
        6, 3, 7,
        6, 0, 3,
        // top face
        3, 2, 5,
        3, 5, 7,
        // back face
        4, 7, 5,
        4, 6, 7,
        // bottom face
        0, 6, 4,
        0, 4, 1,
    ]
}

fn main() {
    let files_in_directory = std::fs::read_dir(DIR_NAME);
    if let Err(error) = files_in_directory {
        println!("Error in reading directiory: {}", error);
        return;
    }

    let files_in_directory = files_in_directory.unwrap();
    let files_in_directory = files_in_directory
        .filter(|entry| {
            entry
                .as_ref()
                .is_ok_and(|element| match element.file_type() {
                    Ok(val) => val.is_file(),
                    _ => false,
                })
        })
        .collect::<Result<Vec<_>, _>>();

    let files_in_directory: Vec<DirEntry> = files_in_directory
        .expect("Could not convert Result vec of DICOM files into Vec of dicom files.");

    let mut dimensions: [usize; 3] = [0; 3];
    let mut data = Vec::new();

    let options = ConvertOptions::new()
        .with_voi_lut(VoiLutOption::Normalize)
        .force_8bit();

    for file in files_in_directory {
        let obj = open_file(file.path()).unwrap();

        let image_type = obj.element_by_name("ImageType").unwrap().to_str().unwrap();

        // Retain just the primary axial(s)
        if !image_type.contains("\\PRIMARY\\AXIAL") {
            continue;
        }

        let position: Vec<f32> = obj
            .element_by_name("ImagePositionPatient")
            .unwrap()
            .to_multi_float32()
            .unwrap();
        let pixel_data = obj
            .decode_pixel_data()
            .expect("Pixel data could not be decoded.");
        let frame_data: Vec<u16> = pixel_data
            .to_vec_with_options(&options)
            .expect("Failed at converting to vec of u8.");

        data.push((
            *position
                .last()
                .expect("Vector should have 3 elements but does not."),
            frame_data,
        ));

        let rows: usize = obj.element(tags::ROWS).unwrap().to_int().unwrap();
        let cols: usize = obj.element(tags::COLUMNS).unwrap().to_int().unwrap();

        dimensions[0] = rows;
        dimensions[1] = cols;
    }

    dimensions[2] = data.len();

    // Sort by Z
    data.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    // Find min and max value
    let (mut min, mut max) = (u16::MAX, u16::MIN);
    for slice in &data {
        let slice = &slice.1;
        for element in slice {
            if *element < min {
                min = *element;
            } else if *element > max {
                max = *element;
            }
        }
    }
    println!("Min value: {}, max value: {}", min, max);

    // Normalize and cast to u8
    let data = data
        .iter()
        .flat_map(|(_, slice)| {
            slice
                .iter()
                .map(|element| {
                    ((((element - min) as f32 / (max - min) as f32) * 255.0) as u8).clamp(0, 255)
                })
                .collect::<Vec<u8>>()
        })
        .collect::<Vec<u8>>();

    println!("Length of data: {}", data.len());
    println!(
        "Multiplication of dimensions: {}",
        dimensions[0] * dimensions[1] * dimensions[2]
    );

    println!(
        "Data range: min = {:?}, max = {:?}",
        data.iter().min(),
        data.iter().max()
    );

    // Setup OpenGL
    let sdl = Sdl::init(init::InitFlags::EVERYTHING);

    sdl.set_gl_context_major_version(3).unwrap();
    sdl.set_gl_context_minor_version(3).unwrap();
    sdl.set_gl_profile(video::GlProfile::Core).unwrap();

    #[cfg(target_os = "macos")]
    {
        // Important to use in MacOS to be able to use Core feature set
        sdl.set_gl_context_flags(video::GlContextFlags::FORWARD_COMPATIBLE)
            .unwrap();
    }

    let win_args = video::CreateWinArgs {
        title: "Naive MIP",
        width: 800,
        height: 600,
        allow_high_dpi: true,
        borderless: false,
        resizable: false,
    };

    let win = sdl
        .create_gl_window(win_args)
        .expect("Could not make a window and context");

    // Load up OpenGL functions
    unsafe {
        load_global_gl(&|f_name| win.get_proc_address(f_name));
    }

    // Set clear color
    unsafe {
        glClearColor(0.2, 0.3, 0.3, 1.0);
    }

    // CREATE A CUBE IN Normalized Device Coordinates
    let vertices = get_cube_vertices();
    let indices = get_cube_indices();

    // Create vertex array object
    let mut vao = 0u32;
    unsafe {
        glGenVertexArrays(1, &mut vao);
    }
    assert!(vao != 0);
    glBindVertexArray(vao);

    // Create vertex buffer object
    let mut vbo = 0u32;
    unsafe {
        glGenBuffers(1, &mut vbo);
    }
    assert!(vbo != 0);
    unsafe {
        glBindBuffer(GL_ARRAY_BUFFER, vbo);
    }
    unsafe {
        glBufferData(
            GL_ARRAY_BUFFER,
            (vertices.len() * mem::size_of::<f32>()).try_into().unwrap(),
            vertices.as_ptr().cast(),
            GL_STATIC_DRAW,
        );
    }

    // Create element buffer object
    let mut ebo = 0u32;
    unsafe {
        glGenBuffers(1, &mut ebo);
    }
    assert!(ebo != 0);
    unsafe {
        glBindBuffer(GL_ELEMENT_ARRAY_BUFFER, ebo);
    }
    unsafe {
        glBufferData(
            GL_ELEMENT_ARRAY_BUFFER,
            (indices.len() * mem::size_of::<f32>()).try_into().unwrap(),
            indices.as_ptr().cast(),
            GL_STATIC_DRAW,
        );
    }

    // Create a 3D texture
    let mut texture_scan = 0u32;
    unsafe {
        glGenTextures(1, &mut texture_scan);
    }
    assert!(texture_scan != 0);
    unsafe {
        glBindTexture(GL_TEXTURE_3D, texture_scan);
        glTexParameteri(
            GL_TEXTURE_3D,
            GL_TEXTURE_WRAP_S,
            GL_CLAMP_TO_BORDER.0 as i32,
        );
        glTexParameteri(
            GL_TEXTURE_3D,
            GL_TEXTURE_WRAP_T,
            GL_CLAMP_TO_BORDER.0 as i32,
        );
        glTexParameteri(
            GL_TEXTURE_3D,
            GL_TEXTURE_WRAP_R,
            GL_CLAMP_TO_BORDER.0 as i32,
        );

        let border_color: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
        glTexParameterfv(
            GL_TEXTURE_3D,
            GL_TEXTURE_BORDER_COLOR,
            border_color.as_ptr().cast(),
        );
        
        // Use GL_NEAREST since we have integer texture
        glTexParameteri(
            GL_TEXTURE_3D,
            GL_TEXTURE_MIN_FILTER,
            GL_NEAREST.0 as i32,
        );
        glTexParameteri(GL_TEXTURE_3D, GL_TEXTURE_MAG_FILTER, GL_NEAREST.0 as i32);

        glTexImage3D(
            GL_TEXTURE_3D,
            0,
            GL_R8.0 as i32,
            dimensions[0] as i32,
            dimensions[1] as i32,
            dimensions[2] as i32,
            0,
            GL_RED,
            GL_UNSIGNED_BYTE,
            data.as_bytes().as_ptr().cast(),
        );

        //glGenerateMipmap(GL_TEXTURE_3D);
        glBindTexture(GL_TEXTURE_3D, 0);
    }

    // Create vertex atribute pointers
    unsafe {
        glVertexAttribPointer(
            0,
            3,
            GL_FLOAT,
            0,
            (6 * mem::size_of::<f32>()).try_into().unwrap(),
            0 as *const _,
        );
        glEnableVertexAttribArray(0);

        glVertexAttribPointer(
            1,
            3,
            GL_FLOAT,
            0,
            (6 * mem::size_of::<f32>()).try_into().unwrap(),
            (3 * mem::size_of::<f32>()) as *const _,
        );
        glEnableVertexAttribArray(1);
    }

    // Unassign everything
    glBindVertexArray(0);
    unsafe {
        glDisableVertexAttribArray(0);
        glDisableVertexAttribArray(1);

        glBindBuffer(GL_ELEMENT_ARRAY_BUFFER, 0);
        glBindBuffer(GL_ARRAY_BUFFER, 0);
    }

    // Create shader program
    const VERT_SHADER: &str = r#"#version 330 core
        layout (location = 0) in vec3 pos;
        layout (location = 1) in vec3 texCoord;

        uniform mat4 transform;

        out vec3 ndcPosition;
        out vec3 textureCoordinates;

        void main() {
            gl_Position = transform * vec4(pos, 1.0); // W is used by the projection transform
            ndcPosition = gl_Position.xyz;
            textureCoordinates = texCoord;
        }
    "#;

    const FRAG_SHADER: &str = r#"#version 330 core

        uniform sampler3D tex3d;
        uniform mat4 transform;
        uniform mat4 inv_transform;

        out vec4 final_color;

        in vec3 ndcPosition;
        in vec3 textureCoordinates;

        void main() {
            vec3 ray_dir = -normalize(ndcPosition - vec3(0.0, 0.0, -1.0));
            vec3 ray_origin = ndcPosition;

            float step_size = 1.0 / 256.0;
            float t = 0.0;
            float max_val = 0.0;
            vec3 tex_sample_pos = vec3(0.0);

            for (int i = 0; i < 256; ++i) {
                vec3 sample_pos = ray_origin + ray_dir * t;
                tex_sample_pos = (inv_transform * vec4(sample_pos, 1.0)).xyz;
                tex_sample_pos = vec4((tex_sample_pos + 0.3)/0.6, 1.0).xyz;

                vec4 v = texture(tex3d, tex_sample_pos);
                max_val = max(max_val, v.r);
                t += step_size;
            }

            final_color = vec4(vec3(max_val), 1.0);
        }
    "#;

    let vs = glCreateShader(GL_VERTEX_SHADER);
    assert!(vs != 0);
    unsafe {
        glShaderSource(
            vs,
            1,
            &(VERT_SHADER.as_bytes().as_ptr().cast()),
            &(VERT_SHADER.len().try_into().unwrap()),
        );
    }
    glCompileShader(vs);
    log_error(vs, true);

    let fs = glCreateShader(GL_FRAGMENT_SHADER);
    assert!(fs != 0);
    unsafe {
        glShaderSource(
            fs,
            1,
            &(FRAG_SHADER.as_bytes().as_ptr().cast()),
            &(FRAG_SHADER.len().try_into().unwrap()),
        );
    }
    glCompileShader(fs);
    log_error(fs, true);

    let program = glCreateProgram();
    assert!(program != 0);
    glAttachShader(program, vs);
    glAttachShader(program, fs);
    glLinkProgram(program);
    log_error(program, false);

    glDeleteShader(vs);
    glDeleteShader(fs);

    // Enable vsync - swap_window blocks until the image has been presented to the user
    // So we show images at most as fast the display's refresh rate
    let _ = win.set_swap_interval(video::GlSwapInterval::Vsync);

    unsafe {
        glEnable(GL_CULL_FACE);
        glCullFace(GL_FRONT);
        glFrontFace(GL_CCW);
    }

    glUseProgram(program);

    let texture_uniform_name = CString::new("tex3d").unwrap();
    let location_texture =
        unsafe { glGetUniformLocation(program, texture_uniform_name.as_ptr().cast()) };
    assert!(location_texture >= 0);
    unsafe {
        glUniform1i(location_texture, 0);

        glActiveTexture(GL_TEXTURE0);
        glBindTexture(GL_TEXTURE_3D, texture_scan);
    }
    
    let transform = CString::new("transform").unwrap();
    let location_transform = unsafe { glGetUniformLocation(program, transform.as_ptr().cast()) };
    assert!(location_transform >= 0);
    let inv_transform = CString::new("inv_transform").unwrap();
    let location_inv_transform = unsafe { glGetUniformLocation(program, inv_transform.as_ptr().cast()) };
    assert!(location_inv_transform >= 0);

    let now = SystemTime::now();

    // Create a program loop
    'main_loop: loop {
        // Handle events this frame
        while let Some(event) = sdl.poll_events() {
            match event {
                (events::Event::Quit, _) => break 'main_loop,
                _ => (),
            }
        }

        unsafe {
            glClear(GL_COLOR_BUFFER_BIT);

            glBindVertexArray(vao);

            let time_value = now.elapsed().unwrap().as_secs_f32();
            let rotation_matrix_y =
                glam::Mat4::from_rotation_y((time_value*0.3) % std::f32::consts::TAU);
            let rotation_matrix_z =
                glam::Mat4::from_rotation_x(-std::f32::consts::TAU/6.0);

            let rotation_matrix = rotation_matrix_y.mul_mat4(&rotation_matrix_z);

            glUniformMatrix4fv(
                location_transform,
                1,
                0,
                rotation_matrix.to_cols_array().as_ptr(),
            );
            glUniformMatrix4fv(
                location_inv_transform,
                1,
                0,
                rotation_matrix.inverse().to_cols_array().as_ptr(),
            );

            glDrawElements(GL_TRIANGLES, 36, GL_UNSIGNED_INT, 0 as *const _);

            win.swap_window();
        }
    }

    unsafe {
        glDeleteVertexArrays(1, &vao);
        glDeleteBuffers(1, &vbo);
        glDeleteBuffers(1, &ebo);
        glDeleteProgram(program);
    }
}

fn log_error(object_id: u32, is_shader: bool) -> () {
    let mut success = 0;

    unsafe {
        if is_shader {
            glGetShaderiv(object_id, GL_COMPILE_STATUS, &mut success);
        } else {
            glGetProgramiv(object_id, GL_LINK_STATUS, &mut success);
        }

        if success == 0 {
            let mut log_len = 0i32;

            if is_shader {
                glGetShaderiv(object_id, GL_INFO_LOG_LENGTH, &mut log_len);
            } else {
                glGetProgramiv(object_id, GL_INFO_LOG_LENGTH, &mut log_len);
            }

            let mut log_message: Vec<u8> = Vec::with_capacity(log_len as usize);

            if is_shader {
                glGetShaderInfoLog(
                    object_id,
                    log_message.capacity() as i32,
                    &mut log_len,
                    log_message.as_mut_ptr().cast(),
                );
            } else {
                glGetProgramInfoLog(
                    object_id,
                    log_message.capacity() as i32,
                    &mut log_len,
                    log_message.as_mut_ptr().cast(),
                );
            }

            log_message.set_len(log_len.try_into().unwrap());

            if is_shader {
                glDeleteShader(object_id);
            }

            panic!(
                "Shader Program Link Error: {}",
                String::from_utf8_lossy(&log_message)
            );
        }
    }
}
