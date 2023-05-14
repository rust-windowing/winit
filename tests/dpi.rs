// Tests for the DPI functionality of the library.

use std::collections::HashSet;

use winit::dpi;

macro_rules! test_pixel_int_impl {
    ($($name:ident => $ty:ty),*) => {$(
        #[test]
        fn $name() {
            use dpi::Pixel;

            assert_eq!(
                <$ty as Pixel>::from_f64(37.0),
                37,
            );
            assert_eq!(
                <$ty as Pixel>::from_f64(37.4),
                37,
            );
            assert_eq!(
                <$ty as Pixel>::from_f64(37.5),
                38,
            );
            assert_eq!(
                <$ty as Pixel>::from_f64(37.9),
                38,
            );

            assert_eq!(
                <$ty as Pixel>::cast::<u8>(37),
                37,
            );
            assert_eq!(
                <$ty as Pixel>::cast::<u16>(37),
                37,
            );
            assert_eq!(
                <$ty as Pixel>::cast::<u32>(37),
                37,
            );
            assert_eq!(
                <$ty as Pixel>::cast::<i8>(37),
                37,
            );
            assert_eq!(
                <$ty as Pixel>::cast::<i16>(37),
                37,
            );
            assert_eq!(
                <$ty as Pixel>::cast::<i32>(37),
                37,
            );
        }
    )*};
}

test_pixel_int_impl! {
    test_pixel_int_u8 => u8,
    test_pixel_int_u16 => u16,
    test_pixel_int_u32 => u32,
    test_pixel_int_i8 => i8,
    test_pixel_int_i16 => i16
}

macro_rules! assert_approx_eq {
    ($a:expr, $b:expr $(,)?) => {
        assert!(
            ($a - $b).abs() < 0.001,
            "{} is not approximately equal to {}",
            $a,
            $b
        );
    };
}

macro_rules! test_pixel_float_impl {
    ($($name:ident => $ty:ty),*) => {$(
        #[test]
        fn $name() {
            use dpi::Pixel;

            assert_approx_eq!(
                <$ty as Pixel>::from_f64(37.0),
                37.0,
            );
            assert_approx_eq!(
                <$ty as Pixel>::from_f64(37.4),
                37.4,
            );
            assert_approx_eq!(
                <$ty as Pixel>::from_f64(37.5),
                37.5,
            );
            assert_approx_eq!(
                <$ty as Pixel>::from_f64(37.9),
                37.9,
            );

            assert_eq!(
                <$ty as Pixel>::cast::<u8>(37.0),
                37,
            );
            assert_eq!(
                <$ty as Pixel>::cast::<u8>(37.4),
                37,
            );
            assert_eq!(
                <$ty as Pixel>::cast::<u8>(37.5),
                38,
            );

            assert_eq!(
                <$ty as Pixel>::cast::<u16>(37.0),
                37,
            );
            assert_eq!(
                <$ty as Pixel>::cast::<u16>(37.4),
                37,
            );
            assert_eq!(
                <$ty as Pixel>::cast::<u16>(37.5),
                38,
            );

            assert_eq!(
                <$ty as Pixel>::cast::<u32>(37.0),
                37,
            );
            assert_eq!(
                <$ty as Pixel>::cast::<u32>(37.4),
                37,
            );
            assert_eq!(
                <$ty as Pixel>::cast::<u32>(37.5),
                38,
            );

            assert_eq!(
                <$ty as Pixel>::cast::<i8>(37.0),
                37,
            );
            assert_eq!(
                <$ty as Pixel>::cast::<i8>(37.4),
                37,
            );
            assert_eq!(
                <$ty as Pixel>::cast::<i8>(37.5),
                38,
            );

            assert_eq!(
                <$ty as Pixel>::cast::<i16>(37.0),
                37,
            );
            assert_eq!(
                <$ty as Pixel>::cast::<i16>(37.4),
                37,
            );
            assert_eq!(
                <$ty as Pixel>::cast::<i16>(37.5),
                38,
            );
        }
    )*};
}

test_pixel_float_impl! {
    test_pixel_float_f32 => f32,
    test_pixel_float_f64 => f64
}

#[test]
fn test_validate_scale_factor() {
    assert!(dpi::validate_scale_factor(1.0));
    assert!(dpi::validate_scale_factor(2.0));
    assert!(dpi::validate_scale_factor(3.0));
    assert!(dpi::validate_scale_factor(1.5));
    assert!(dpi::validate_scale_factor(0.5));

    assert!(!dpi::validate_scale_factor(0.0));
    assert!(!dpi::validate_scale_factor(-1.0));
    assert!(!dpi::validate_scale_factor(f64::INFINITY));
    assert!(!dpi::validate_scale_factor(f64::NAN));
    assert!(!dpi::validate_scale_factor(f64::NEG_INFINITY));
}

#[test]
fn test_logical_position() {
    let log_pos = dpi::LogicalPosition::new(1.0, 2.0);
    assert_eq!(
        log_pos.to_physical::<u32>(1.0),
        dpi::PhysicalPosition::new(1, 2)
    );
    assert_eq!(
        log_pos.to_physical::<u32>(2.0),
        dpi::PhysicalPosition::new(2, 4)
    );
    assert_eq!(log_pos.cast::<u32>(), dpi::LogicalPosition::new(1, 2));
    assert_eq!(
        log_pos,
        dpi::LogicalPosition::from_physical(dpi::PhysicalPosition::new(1.0, 2.0), 1.0)
    );
    assert_eq!(
        log_pos,
        dpi::LogicalPosition::from_physical(dpi::PhysicalPosition::new(2.0, 4.0), 2.0)
    );
    assert_eq!(
        dpi::LogicalPosition::from((2.0, 2.0)),
        dpi::LogicalPosition::new(2.0, 2.0)
    );
    assert_eq!(
        dpi::LogicalPosition::from([2.0, 3.0]),
        dpi::LogicalPosition::new(2.0, 3.0)
    );

    let x: (f64, f64) = log_pos.into();
    assert_eq!(x, (1.0, 2.0));
    let x: [f64; 2] = log_pos.into();
    assert_eq!(x, [1.0, 2.0]);
}

#[test]
fn test_physical_position() {
    assert_eq!(
        dpi::PhysicalPosition::from_logical(dpi::LogicalPosition::new(1.0, 2.0), 1.0),
        dpi::PhysicalPosition::new(1, 2)
    );
    assert_eq!(
        dpi::PhysicalPosition::from_logical(dpi::LogicalPosition::new(2.0, 4.0), 0.5),
        dpi::PhysicalPosition::new(1, 2)
    );
    assert_eq!(
        dpi::PhysicalPosition::from((2.0, 2.0)),
        dpi::PhysicalPosition::new(2.0, 2.0)
    );
    assert_eq!(
        dpi::PhysicalPosition::from([2.0, 3.0]),
        dpi::PhysicalPosition::new(2.0, 3.0)
    );

    let x: (f64, f64) = dpi::PhysicalPosition::new(1, 2).into();
    assert_eq!(x, (1.0, 2.0));
    let x: [f64; 2] = dpi::PhysicalPosition::new(1, 2).into();
    assert_eq!(x, [1.0, 2.0]);
}

#[test]
fn test_logical_size() {
    let log_size = dpi::LogicalSize::new(1.0, 2.0);
    assert_eq!(
        log_size.to_physical::<u32>(1.0),
        dpi::PhysicalSize::new(1, 2)
    );
    assert_eq!(
        log_size.to_physical::<u32>(2.0),
        dpi::PhysicalSize::new(2, 4)
    );
    assert_eq!(log_size.cast::<u32>(), dpi::LogicalSize::new(1, 2));
    assert_eq!(
        log_size,
        dpi::LogicalSize::from_physical(dpi::PhysicalSize::new(1.0, 2.0), 1.0)
    );
    assert_eq!(
        log_size,
        dpi::LogicalSize::from_physical(dpi::PhysicalSize::new(2.0, 4.0), 2.0)
    );
    assert_eq!(
        dpi::LogicalSize::from((2.0, 2.0)),
        dpi::LogicalSize::new(2.0, 2.0)
    );
    assert_eq!(
        dpi::LogicalSize::from([2.0, 3.0]),
        dpi::LogicalSize::new(2.0, 3.0)
    );

    let x: (f64, f64) = log_size.into();
    assert_eq!(x, (1.0, 2.0));
    let x: [f64; 2] = log_size.into();
    assert_eq!(x, [1.0, 2.0]);
}

#[test]
fn test_physical_size() {
    assert_eq!(
        dpi::PhysicalSize::from_logical(dpi::LogicalSize::new(1.0, 2.0), 1.0),
        dpi::PhysicalSize::new(1, 2)
    );
    assert_eq!(
        dpi::PhysicalSize::from_logical(dpi::LogicalSize::new(2.0, 4.0), 0.5),
        dpi::PhysicalSize::new(1, 2)
    );
    assert_eq!(
        dpi::PhysicalSize::from((2.0, 2.0)),
        dpi::PhysicalSize::new(2.0, 2.0)
    );
    assert_eq!(
        dpi::PhysicalSize::from([2.0, 3.0]),
        dpi::PhysicalSize::new(2.0, 3.0)
    );

    let x: (f64, f64) = dpi::PhysicalSize::new(1, 2).into();
    assert_eq!(x, (1.0, 2.0));
    let x: [f64; 2] = dpi::PhysicalSize::new(1, 2).into();
    assert_eq!(x, [1.0, 2.0]);
}

#[test]
fn test_size() {
    assert_eq!(
        dpi::Size::new(dpi::PhysicalSize::new(1, 2)),
        dpi::Size::Physical(dpi::PhysicalSize::new(1, 2))
    );
    assert_eq!(
        dpi::Size::new(dpi::LogicalSize::new(1.0, 2.0)),
        dpi::Size::Logical(dpi::LogicalSize::new(1.0, 2.0))
    );

    assert_eq!(
        dpi::Size::new(dpi::PhysicalSize::new(1, 2)).to_logical::<f64>(1.0),
        dpi::LogicalSize::new(1.0, 2.0)
    );
    assert_eq!(
        dpi::Size::new(dpi::PhysicalSize::new(1, 2)).to_logical::<f64>(2.0),
        dpi::LogicalSize::new(0.5, 1.0)
    );
    assert_eq!(
        dpi::Size::new(dpi::LogicalSize::new(1.0, 2.0)).to_logical::<f64>(1.0),
        dpi::LogicalSize::new(1.0, 2.0)
    );

    assert_eq!(
        dpi::Size::new(dpi::PhysicalSize::new(1, 2)).to_physical::<u32>(1.0),
        dpi::PhysicalSize::new(1, 2)
    );
    assert_eq!(
        dpi::Size::new(dpi::PhysicalSize::new(1, 2)).to_physical::<u32>(2.0),
        dpi::PhysicalSize::new(1, 2)
    );
    assert_eq!(
        dpi::Size::new(dpi::LogicalSize::new(1.0, 2.0)).to_physical::<u32>(1.0),
        dpi::PhysicalSize::new(1, 2)
    );
    assert_eq!(
        dpi::Size::new(dpi::LogicalSize::new(1.0, 2.0)).to_physical::<u32>(2.0),
        dpi::PhysicalSize::new(2, 4)
    );

    let small = dpi::Size::Physical((1, 2).into());
    let medium = dpi::Size::Logical((3, 4).into());
    let medium_physical = dpi::Size::new(medium.to_physical::<u32>(1.0));
    let large = dpi::Size::Physical((5, 6).into());
    assert_eq!(dpi::Size::clamp(medium, small, large, 1.0), medium_physical);
    assert_eq!(dpi::Size::clamp(small, medium, large, 1.0), medium_physical);
    assert_eq!(dpi::Size::clamp(large, small, medium, 1.0), medium_physical);
}

#[test]
fn test_position() {
    assert_eq!(
        dpi::Position::new(dpi::PhysicalPosition::new(1, 2)),
        dpi::Position::Physical(dpi::PhysicalPosition::new(1, 2))
    );
    assert_eq!(
        dpi::Position::new(dpi::LogicalPosition::new(1.0, 2.0)),
        dpi::Position::Logical(dpi::LogicalPosition::new(1.0, 2.0))
    );

    assert_eq!(
        dpi::Position::new(dpi::PhysicalPosition::new(1, 2)).to_logical::<f64>(1.0),
        dpi::LogicalPosition::new(1.0, 2.0)
    );
    assert_eq!(
        dpi::Position::new(dpi::PhysicalPosition::new(1, 2)).to_logical::<f64>(2.0),
        dpi::LogicalPosition::new(0.5, 1.0)
    );
    assert_eq!(
        dpi::Position::new(dpi::LogicalPosition::new(1.0, 2.0)).to_logical::<f64>(1.0),
        dpi::LogicalPosition::new(1.0, 2.0)
    );

    assert_eq!(
        dpi::Position::new(dpi::PhysicalPosition::new(1, 2)).to_physical::<u32>(1.0),
        dpi::PhysicalPosition::new(1, 2)
    );
    assert_eq!(
        dpi::Position::new(dpi::PhysicalPosition::new(1, 2)).to_physical::<u32>(2.0),
        dpi::PhysicalPosition::new(1, 2)
    );
    assert_eq!(
        dpi::Position::new(dpi::LogicalPosition::new(1.0, 2.0)).to_physical::<u32>(1.0),
        dpi::PhysicalPosition::new(1, 2)
    );
    assert_eq!(
        dpi::Position::new(dpi::LogicalPosition::new(1.0, 2.0)).to_physical::<u32>(2.0),
        dpi::PhysicalPosition::new(2, 4)
    );
}

// Eat coverage for the Debug impls et al
#[test]
fn attr_coverage() {
    let _ = format!("{:?}", dpi::LogicalPosition::<u32>::default().clone());
    HashSet::new().insert(dpi::LogicalPosition::<u32>::default());

    let _ = format!("{:?}", dpi::PhysicalPosition::<u32>::default().clone());
    HashSet::new().insert(dpi::PhysicalPosition::<u32>::default());

    let _ = format!("{:?}", dpi::LogicalSize::<u32>::default().clone());
    HashSet::new().insert(dpi::LogicalSize::<u32>::default());

    let _ = format!("{:?}", dpi::PhysicalSize::<u32>::default().clone());
    HashSet::new().insert(dpi::PhysicalSize::<u32>::default());

    let _ = format!("{:?}", dpi::Size::Physical((1, 2).into()).clone());
    let _ = format!("{:?}", dpi::Position::Physical((1, 2).into()).clone());
}
