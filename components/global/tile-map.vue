<template>
    <div class="map">
        <a href="https://goo.gl/maps/imh7tx5GP7UdeuEt8" target="_blank">
            <img src="/images/map.jpg" />
        </a>
    </div>
</template>

<style lang="scss" scoped>

.map {
    border-radius: 30px;
    position: relative;
    padding-top: 75%;
    overflow: hidden;
    margin-top: 30px;
    background: #37495A;

    &:after {
        content: 'Open Google map';
        opacity: 0;
        position: absolute;
        display: block;
        width: 100%;
        text-align: center;
        top: 45%;
        left: 0;
        color: #fff;
        font-weight: bold;
        z-index: 4;
        transition: 0.5s ease opacity;
    }

    &:hover {
        &:after { opacity: 1; }
        & img { opacity: 0.3; }
    }

    img {
        position: absolute;
        width: 100%;
        top: -19%;
        left: 0;
        opacity: 1;
        transition: 0.5s ease opacity;
        &:hover {
            opacity: 0.3;
        }
    }

}


</style>